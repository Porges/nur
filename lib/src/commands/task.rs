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
use tokio_util::sync::CancellationToken;

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

        let graph = {
            let mut graph: DiGraphMap<&str, ()> = DiGraphMap::new();
            for (name, data) in &config.tasks {
                graph.add_node(name);
                for dep in &data.dependencies {
                    graph.add_edge(name, dep, ());
                }
            }
            graph
        };

        // validate no cycles in graph
        algo::toposort(&graph, None).map_err(|cyc| crate::Error::TaskCycle {
            path,
            task_name: cyc.node_id().to_owned(),
        })?;

        let execution_order = get_execution_order(graph, &self.task_names);

        let longest_name = execution_order.iter().map(|x| x.len()).max();
        let output = crate::output::from_config(&config, longest_name);

        let (tx, rx) = mpsc::channel::<crate::StatusMessage>(100);
        let local_ctx = LocalContext {
            cwd: ctx.cwd.clone(),
            tx,
        };

        let (task_results, ()) = tokio::join!(
            run_tasks(local_ctx, execution_order, &config.tasks),
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

fn get_execution_order<'a>(
    graph: DiGraphMap<&'a str, ()>,
    tasks: &'a BTreeSet<String>,
) -> Vec<&'a str> {
    let mut to_visit = {
        if tasks.is_empty() {
            VecDeque::from([DEFAULT_TASK_NAME])
        } else {
            VecDeque::from_iter(tasks.iter().map(|x| x.as_str()))
        }
    };

    let mut visitor = petgraph::visit::DfsPostOrder::new(
        &graph,
        to_visit
            .pop_front()
            .expect("always at least one in to_visit"),
    );

    // build the execution order for the graph
    // this iterates from the first to_visit member
    let mut run_order = Vec::new();
    while let Some(nx) = visitor.next(&graph) {
        run_order.push(nx);
    }

    // now visit the rest of the to_visit members
    while let Some(start) = to_visit.pop_front() {
        visitor.move_to(start);
        while let Some(nx) = visitor.next(&graph) {
            run_order.push(nx);
        }
    }

    run_order
}

#[derive(Clone)]
struct LocalContext {
    cwd: std::path::PathBuf,
    tx: mpsc::Sender<crate::StatusMessage>,
}

async fn run_tasks(
    ctx: LocalContext,
    run_order: Vec<&str>,
    tasks: &BTreeMap<String, NurTask>,
) -> miette::Result<Vec<miette::Result<TaskResult>>> {
    let cancellation = CancellationToken::new();
    let mut spawned = Vec::with_capacity(run_order.len());

    let mut so_far: BTreeMap<&str, Shared<oneshot::Receiver<()>>> = BTreeMap::new();
    for task_name in run_order {
        let task = tasks
            .get(task_name)
            .ok_or_else(|| crate::Error::NoSuchTask {
                task_name: task_name.to_string(),
            })?;

        // get receivers for all dependencies:
        let mut await_on = Vec::new();
        for dependency in &task.dependencies {
            let recvr =
                so_far
                    .get(dependency.as_str())
                    .ok_or_else(|| crate::Error::NoSuchTask {
                        task_name: dependency.to_string(),
                    })?;

            await_on.push(recvr.clone());
        }

        let (sender, receiver) = oneshot::channel();
        so_far.insert(task_name, receiver.shared());

        spawned.push(run_task(
            ctx.clone(),
            cancellation.clone(),
            await_on,
            task_name,
            task,
            sender,
        ));
    }

    Ok(futures::future::join_all(spawned).await)
}

/// Executes a single task and emits start/stop events.
async fn run_task(
    ctx: LocalContext,
    cancellation: CancellationToken,
    await_on: Vec<Shared<oneshot::Receiver<()>>>,
    task_name: &str,
    task: &NurTask,
    done: oneshot::Sender<()>,
) -> miette::Result<TaskResult> {
    // if upstream task failed it will not trigger its "done" sender,
    // and we will bail out, and thus will also not send a result
    if futures::future::try_join_all(await_on).await.is_err() {
        // don’t report this as an error; task cancelled
        let result = TaskResult::Skipped;
        ctx.tx
            .send(StatusMessage::TaskFinished {
                name: task_name.to_string(),
                result: Ok(result),
            })
            .await
            .into_diagnostic()?;
        return Ok(result);
    }

    ctx.tx
        .send(StatusMessage::TaskStarted {
            name: task_name.to_string(),
        })
        .await
        .into_diagnostic()?;

    let result = run_cmds(&ctx, task_name, task, &cancellation).await;
    if let Ok(TaskResult::RanToCompletion) = result {
        // trigger dependent tasks,
        // ignore failures from downstream tasks not existing
        let _ = done.send(());
    }

    ctx.tx
        .send(StatusMessage::TaskFinished {
            name: task_name.to_string(),
            result: result.clone(),
        })
        .await
        .into_diagnostic()?;

    Ok(result.map_err(|e| crate::Error::TaskFailed {
        task_name: task_name.to_string(),
        task_error: e,
    })?)
}

/// Executes the commands for a single task.
async fn run_cmds(
    ctx: &LocalContext,
    task_name: &str,
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
            .envs(&task.env) // task environment is overridden by cmd
            .envs(&cmd.env)
            .group_spawn()
            .map_err(|e| TaskError::IoError { kind: e.kind() })?;

        let ((), (), status) = tokio::join!(
            spawn_reader(
                child.inner().stdout.take().expect("no stdout handle"),
                ctx.tx.clone(),
                |line| StatusMessage::StdOut {
                    task_name: task_name.to_string(),
                    line
                },
            ),
            spawn_reader(
                child.inner().stderr.take().expect("no stderr handle"),
                ctx.tx.clone(),
                |line| StatusMessage::StdErr {
                    task_name: task_name.to_string(),
                    line
                },
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
