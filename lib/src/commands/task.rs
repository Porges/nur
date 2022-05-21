use std::collections::{BTreeMap, BTreeSet, VecDeque};

use command_group::{tokio::AsyncCommandGroup, UnixChildExt};
use futures::{future::Shared, FutureExt};
use miette::IntoDiagnostic;
use petgraph::algo;
use petgraph::graphmap::DiGraphMap;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    sync::{mpsc, oneshot},
};

use crate::nurfile::NurTask;

pub struct Task {
    pub dry_run: bool,
    pub nur_file: Option<std::path::PathBuf>,
    pub task_names: BTreeSet<String>,
}

const DEFAULT_TASK_NAME: &str = "default";

#[async_trait::async_trait]
impl crate::commands::Command for Task {
    async fn run(&self, ctx: crate::commands::Context) -> miette::Result<()> {
        let (_, config) = crate::load_config(&ctx.cwd, &self.nur_file)?;

        // validate no cycles
        {
            let mut graph: DiGraphMap<&str, ()> = DiGraphMap::new();
            for (name, data) in &config.tasks {
                graph.add_node(name);
                for dep in &data.dependencies {
                    graph.add_edge(name, dep, ());
                }
            }

            algo::toposort(&graph, None).map_err(|cyc| crate::Error::TaskCycle {
                task_name: cyc.node_id().to_owned(),
            })?;
        }

        // TODO: report all errors?
        for result in run_tasks(&ctx, &self.task_names, config.tasks).await? {
            match result {
                Ok(_) => {}
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }
}

struct TaskToRun {
    task_name: String,
    task: crate::nurfile::NurTask,
    sender: oneshot::Sender<()>,
}

struct Runner {
    receivers: BTreeMap<String, Shared<oneshot::Receiver<()>>>,
    to_run: VecDeque<TaskToRun>,
    tasks: BTreeMap<String, NurTask>,
}

impl Runner {
    fn new(tasks: BTreeMap<String, NurTask>) -> Self {
        Runner {
            receivers: Default::default(),
            to_run: Default::default(),
            tasks,
        }
    }

    pub fn enqueue_task(
        &mut self,
        task_name: &str,
    ) -> miette::Result<&Shared<oneshot::Receiver<()>>> {
        if self.receivers.contains_key(task_name) {
            Ok(&self.receivers[task_name])
        } else {
            let (task_name, task) =
                self.tasks
                    .remove_entry(task_name)
                    .ok_or_else(|| crate::Error::NoSuchTask {
                        task_name: task_name.to_string(),
                    })?;

            let (sender, receiver) = oneshot::channel();
            self.receivers.insert(task_name.clone(), receiver.shared());
            let result = &self.receivers[&task_name];
            self.to_run.push_back(TaskToRun {
                task_name,
                task,
                sender,
            });
            Ok(result)
        }
    }
}

#[derive(PartialEq)]
enum TaskResult {
    Skipped,
    Cancelled,
    RanToCompletion,
}

async fn run_tasks(
    ctx: &crate::commands::Context,
    task_names: &BTreeSet<String>,
    tasks: BTreeMap<String, NurTask>,
) -> miette::Result<Vec<miette::Result<TaskResult>>> {
    let mut r = Runner::new(tasks);
    if task_names.is_empty() {
        r.enqueue_task(DEFAULT_TASK_NAME)?;
    } else {
        for task_name in task_names {
            r.enqueue_task(task_name)?;
        }
    }

    let mut spawned: Vec<_> = Vec::new();

    let cancellation = tokio_util::sync::CancellationToken::new();

    while let Some(run) = r.to_run.pop_front() {
        let ctx = ctx.clone();
        let mut await_upon = Vec::new();
        for dep in &run.task.dependencies {
            await_upon.push(r.enqueue_task(dep)?.clone());
        }

        let cancellation = cancellation.clone();
        spawned.push(async move {
            // if upstream task failed it will not send a result,
            // and we will bail out, and thus will also not send a result
            if futures::future::try_join_all(await_upon).await.is_err() {
                // don’t report this as an error; task cancelled
                return Ok(TaskResult::Skipped);
            }

            let result = run_cmds(ctx, run.task_name, &run.task, &cancellation).await?;
            if result == TaskResult::RanToCompletion {
                // ignore failures from downstream tasks not existing
                let _ = run.sender.send(());
            }

            Ok(result)
        });
    }

    Ok(futures::future::join_all(spawned).await)
}

async fn run_cmds(
    ctx: crate::commands::Context,
    task_name: String,
    task: &NurTask,
    cancellation: &tokio_util::sync::CancellationToken,
) -> miette::Result<TaskResult> {
    for cmd in &task.commands {
        let mut child = tokio::process::Command::new("/bin/sh")
            .args(["-c", &cmd.sh])
            .current_dir(&ctx.cwd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .envs(&cmd.env)
            .group_spawn()
            .into_diagnostic()?;

        let ((), (), status) = tokio::join!(
            spawn_reader(
                child.inner().stdout.take().expect("no stdout handle"),
                ctx.stdout.clone()
            ),
            spawn_reader(
                child.inner().stderr.take().expect("no stderr handle"),
                ctx.stderr.clone()
            ),
            async move {
                tokio::select! {
                    () = cancellation.cancelled() => {
                        let _ = child.signal(command_group::Signal::SIGINT);
                        _ = child.wait().await;
                        None
                    }
                    result = child.wait() => {
                        Some(result)
                    }
                }
            },
        );

        if let Some(status) = status {
            let status = status.into_diagnostic()?;
            if !cmd.ignore_result && status.code() != Some(0) {
                cancellation.cancel();
                return Err(crate::Error::TaskFailed {
                    task_name,
                    command: cmd.sh.clone(),
                    exit_status: status,
                }
                .into());
            }
        } else {
            // we were cancelled
            return Ok(TaskResult::Cancelled);
        }
    }

    Ok(TaskResult::RanToCompletion)
}

async fn spawn_reader<R>(from: R, into: mpsc::Sender<String>)
where
    R: AsyncRead + Send + 'static,
    BufReader<R>: Unpin,
{
    let mut reader = BufReader::new(from).lines();
    while let Ok(Some(line)) = reader.next_line().await {
        if (into.send(line).await).is_err() {
            break;
        }
    }
}
