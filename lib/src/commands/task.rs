use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;

use futures::{future::Shared, FutureExt};
use miette::IntoDiagnostic;
use petgraph::graphmap::DiGraphMap;
use process_wrap::tokio::TokioCommandWrap;
use rustworkx_core::connectivity::find_cycle;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    sync::{mpsc, oneshot},
};
use tokio_util::sync::CancellationToken;

use crate::nurfile::NurFile;
use crate::{
    nurfile::{NurTask, OutputOptions},
    Error, StatusMessage, TaskError, TaskResult, TaskStatus,
};

pub struct Task {
    pub dry_run: bool,
    pub nur_file: Option<std::path::PathBuf>,
    pub task_names: BTreeSet<String>,
    pub output_override: Option<OutputOptions>,
}

const DEFAULT_TASK_NAME: &str = "default";

impl crate::commands::Command for Task {
    fn run(&self, ctx: crate::commands::Context) -> miette::Result<()> {
        let (path, config) = crate::nurfile::load_config(&ctx.cwd, self.nur_file.as_deref())?;

        let execution_order = self.tasks_from_config(path, &config)?;

        if self.dry_run {
            ctx.stdout
                .write_all("Would run tasks in the following order:\n".as_bytes())
                .into_diagnostic()?;

            for task_name in execution_order {
                let msg = format!("- {task_name}\n");
                ctx.stdout.write_all(msg.as_bytes()).into_diagnostic()?;
            }

            ctx.stdout.flush().into_diagnostic()?;
            Ok(())
        } else {
            let mut output = crate::output::create(
                ctx.stdout,
                ctx.stderr,
                self.output_override
                    .as_ref()
                    .unwrap_or(&config.options.output),
                &execution_order,
            );

            let (tx, mut rx) = mpsc::channel::<crate::StatusMessage>(100);
            let local_ctx = LocalContext {
                cwd: ctx.cwd.clone(),
                tx,
            };

            let task_results = std::thread::scope(|s| {
                // one thread for all tasks to run on
                let result = s.spawn(|| {
                    let tokio_rt = tokio::runtime::Builder::new_current_thread()
                        .enable_io()
                        .build()
                        .unwrap();

                    tokio_rt
                        .block_on(run_tasks(local_ctx, execution_order, &config.tasks))
                        .unwrap()
                });

                // do I/O on main thread
                while let Some(msg) = rx.blocking_recv() {
                    output.handle(msg);
                }

                result.join().unwrap()
            });

            let failures =
                Vec::from_iter(task_results.into_iter().filter_map(|result| result.err()));

            if failures.len() == 1 {
                Err(failures.into_iter().next().unwrap().into())
            } else if failures.is_empty() {
                Ok(())
            } else {
                Err(Error::Multiple { failures }.into())
            }
        }
    }
}

impl Task {
    fn tasks_from_config<'a>(
        &'a self,
        path: PathBuf,
        config: &'a NurFile,
    ) -> crate::Result<Vec<&'a str>> {
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
        let initial_task = config.tasks.keys().next().map(|s| s.as_str());
        let cycle = find_cycle(&graph, initial_task);
        if !cycle.is_empty() {
            let task_names = cycle
                .into_iter()
                .map(|(from, _to)| from.to_string())
                .collect();

            let cycle = crate::Cycle { path: task_names };
            return Err(crate::Error::TaskCycle { path, cycle });
        }

        Ok(get_execution_order(graph, &self.task_names))
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
) -> miette::Result<Vec<crate::Result<TaskResult>>> {
    let cancellation = CancellationToken::new();
    let mut spawned = Vec::with_capacity(run_order.len());
    {
        let mut so_far: BTreeMap<&str, Shared<oneshot::Receiver<()>>> = BTreeMap::new();
        for (task_id, task_name) in run_order.into_iter().enumerate() {
            let task = tasks
                .get(task_name)
                .ok_or_else(|| crate::Error::NoSuchTask {
                    task_name: task_name.to_string(),
                })?;

            // get receivers for all dependencies:
            let mut await_on = Vec::with_capacity(task.dependencies.len());
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
                task_id,
                task_name,
                task,
                sender,
            ));
        }
    }

    Ok(futures::future::join_all(spawned).await)
}

/// Executes a single task and emits start/stop events.
async fn run_task(
    ctx: LocalContext,
    cancellation: CancellationToken,
    await_on: Vec<Shared<oneshot::Receiver<()>>>,
    task_id: usize,
    task_name: &str,
    task: &NurTask,
    done: oneshot::Sender<()>,
) -> miette::Result<TaskResult, crate::Error> {
    // if upstream task failed it will not trigger its "done" sender,
    // and we will bail out, and thus will also not send a result
    if futures::future::try_join_all(await_on).await.is_err() {
        // don’t report this as an error; task cancelled
        let result = TaskResult::Skipped;
        ctx.tx
            .send((task_id, TaskStatus::Finished { result: Ok(result) }))
            .await
            .map_err(crate::internal_error)?;

        return Ok(result);
    }

    ctx.tx
        .send((task_id, TaskStatus::Started {}))
        .await
        .map_err(crate::internal_error)?;

    let result = run_cmds(&ctx, task_id, task, &cancellation).await;
    if let Ok(TaskResult::RanToCompletion) = result {
        // trigger dependent tasks,
        // ignore failures from downstream tasks not existing
        let _ = done.send(());
    } else {
        // don't trigger dependent tasks
        drop(done);
    }

    ctx.tx
        .send((
            task_id,
            TaskStatus::Finished {
                result: result.clone(),
            },
        ))
        .await
        .map_err(crate::internal_error)?;

    result.map_err(|task_error| crate::Error::TaskFailed {
        task_name: task_name.to_string(),
        task_error,
    })
}

/// Executes the commands for a single task.
async fn run_cmds(
    ctx: &LocalContext,
    task_id: usize,
    task: &NurTask,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<TaskResult, TaskError> {
    for cmd in &task.commands {
        // last-chance check before starting process
        if cancellation.is_cancelled() {
            return Ok(TaskResult::Cancelled);
        }

        let shell = "/bin/sh";

        let mut wrapper = TokioCommandWrap::with_new(shell, |c| {
            c.args(["-c", &cmd.sh])
                .current_dir(&ctx.cwd)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .envs(&task.env) // task environment is overridden by cmd
                .envs(&cmd.env);
        });

        #[cfg(target_os = "windows")]
        wrapper.wrap(process_wrap::tokio::JobObject::new());

        // TODO: this still isn't going to compile yet

        #[cfg(target_os = "linux")]
        wrapper.wrap(process_wrap::tokio::ProcessGroup::leader());

        let mut child = wrapper.spawn().map_err(|e| TaskError::ExecutableError {
            executable: shell.to_string(),
            kind: e.kind(),
        })?;

        let stdout = child.inner_mut().stdout.take().expect("no stdout handle");
        let stderr = child.inner_mut().stderr.take().expect("no stderr handle");

        let ((), (), status) = tokio::join!(
            spawn_reader(stdout, ctx.tx.clone(), task_id, TaskStatus::StdOut),
            spawn_reader(stderr, ctx.tx.clone(), task_id, TaskStatus::StdErr),
            async move {
                tokio::select! {
                    () = cancellation.cancelled(), if task.cancellable => {
                        if let Err(e) = child.signal(2 /* SIGINT */) {
                            if e.kind() == std::io::ErrorKind::InvalidInput {
                                // already exited
                                return Some(Box::into_pin(child.wait()).await);
                            }
                        }
                        _ = Box::into_pin(child.wait()).await;
                        None
                    }
                    result = Box::into_pin(child.wait()) => {
                        Some(result)
                    }
                }
            },
        );

        if let Some(status) = status {
            let exit_status = status.map_err(|e| TaskError::ExecutableWaitFailure {
                executable: shell.to_string(),
                kind: e.kind(),
            })?;

            if !cmd.ignore_result && exit_status.code() != Some(0) {
                cancellation.cancel();
                return Err(TaskError::Failed {
                    command: cmd.sh.clone(),
                    exit_status,
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
    task_id: usize,
    f: impl Fn(String) -> TaskStatus,
) where
    R: AsyncRead + Send + 'static,
    BufReader<R>: Unpin,
{
    let mut reader = BufReader::new(from).lines();
    while let Ok(Some(line)) = reader.next_line().await {
        if (into.send((task_id, f(line))).await).is_err() {
            break;
        }
    }
}
