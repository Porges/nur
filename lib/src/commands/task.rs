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

use crate::{nurfile::NurTask, StatusMessage, TaskError, TaskResult};

pub struct Task {
    pub dry_run: bool,
    pub nur_file: Option<std::path::PathBuf>,
    pub task_names: BTreeSet<String>,
}

const DEFAULT_TASK_NAME: &str = "default";

#[async_trait::async_trait]
impl crate::commands::Command for Task {
    async fn run(&self, ctx: crate::commands::Context) -> miette::Result<()> {
        let (path, config) = crate::nurfile::load_config(&ctx.cwd, &self.nur_file)?;

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
                path,
                task_name: cyc.node_id().to_owned(),
            })?;
        }

        let output = crate::output::from_config(&config);

        let (tx, rx) = mpsc::channel::<crate::StatusMessage>(100);
        let local_ctx = LocalContext {
            cwd: ctx.cwd.clone(),
            tx,
        };

        let (task_results, ()) = tokio::join!(
            run_tasks(local_ctx, &self.task_names, config.tasks),
            output.handle(&ctx, rx),
        );

        // TODO: report all errors?
        for result in task_results? {
            match result {
                Ok(_) => {}
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }
}

#[derive(Clone)]
struct LocalContext {
    cwd: std::path::PathBuf,
    tx: mpsc::Sender<crate::StatusMessage>,
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

async fn run_tasks(
    ctx: LocalContext,
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
                let result = TaskResult::Skipped;
                ctx.tx
                    .send(StatusMessage::TaskFinished {
                        name: run.task_name,
                        result: Ok(result),
                    })
                    .await
                    .into_diagnostic()?;
                return Ok(result);
            }

            ctx.tx
                .send(StatusMessage::TaskStarted {
                    name: run.task_name.clone(),
                })
                .await
                .into_diagnostic()?;

            let result = run_cmds(&ctx, &run.task, &cancellation).await;
            if let Ok(TaskResult::RanToCompletion) = result {
                // trigger dependent tasks,
                // ignore failures from downstream tasks not existing
                let _ = run.sender.send(());
            }

            ctx.tx
                .send(StatusMessage::TaskFinished {
                    name: run.task_name.clone(),
                    result: result.clone(),
                })
                .await
                .into_diagnostic()?;

            Ok(result.map_err(|e| crate::Error::TaskFailed {
                task_name: run.task_name,
                task_error: e,
            })?)
        });
    }

    Ok(futures::future::join_all(spawned).await)
}

async fn run_cmds(
    ctx: &LocalContext,
    task: &NurTask,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<TaskResult, TaskError> {
    for cmd in &task.commands {
        // last check before starting process
        if cancellation.is_cancelled() {
            return Ok(TaskResult::Cancelled);
        }

        let mut child = tokio::process::Command::new("/bin/sh")
            .args(["-c", &cmd.sh])
            .current_dir(&ctx.cwd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .envs(&cmd.env)
            .group_spawn()
            .map_err(|e| TaskError::IoError { kind: e.kind() })?;

        let ((), (), status) = tokio::join!(
            spawn_reader(
                child.inner().stdout.take().expect("no stdout handle"),
                ctx.tx.clone(),
                StatusMessage::StdOut,
            ),
            spawn_reader(
                child.inner().stderr.take().expect("no stderr handle"),
                ctx.tx.clone(),
                StatusMessage::StdErr,
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
            let status = status.map_err(|e| TaskError::IoError { kind: e.kind() })?;
            if !cmd.ignore_result && status.code() != Some(0) {
                cancellation.cancel();
                return Err(TaskError::Failed {
                    command: cmd.sh.clone(),
                    exit_status: status,
                });
            }
        } else {
            // we were cancelled
            return Ok(TaskResult::Cancelled);
        }
    }

    Ok(TaskResult::RanToCompletion)
}

async fn spawn_reader<R>(
    from: R,
    into: mpsc::Sender<StatusMessage>,
    f: impl Fn(String) -> StatusMessage,
) where
    R: AsyncRead + Send + 'static,
    BufReader<R>: Unpin,
{
    let mut reader = BufReader::new(from).lines();
    while let Ok(Some(line)) = reader.next_line().await {
        if (into.send(f(line)).await).is_err() {
            break;
        }
    }
}
