use std::{
    collections::{btree_map::Entry, BTreeMap, BTreeSet, VecDeque},
    sync::RwLock,
};

use futures::FutureExt;
use miette::IntoDiagnostic;
use petgraph::algo;
use petgraph::graphmap::DiGraphMap;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::oneshot,
    task::JoinHandle,
};

pub struct Task {
    pub task_names: Vec<String>,
}

async fn run_cmds(
    ctx: crate::commands::Context,
    task_name: String,
    task: &crate::nurfile::Task,
) -> miette::Result<()> {
    for cmd in &task.commands {
        let mut child = tokio::process::Command::new("/bin/sh")
            .args(["-c", &cmd.sh])
            .current_dir(&ctx.cwd)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .into_diagnostic()?;

        let mut reader_out = BufReader::new(child.stdout.take().expect("no stdout handle")).lines();

        let stdout = ctx.stdout.clone();
        let out_task = tokio::spawn(async move {
            while let Ok(Some(line)) = reader_out.next_line().await {
                if (stdout.send(line).await).is_err() {
                    break;
                }
            }
        });

        let mut reader_err = BufReader::new(child.stderr.take().expect("no stderr handle")).lines();

        let stderr = ctx.stderr.clone();
        let err_task = tokio::spawn(async move {
            while let Ok(Some(line)) = reader_err.next_line().await {
                if (stderr.send(line).await).is_err() {
                    break;
                }
            }
        });

        let status = child.wait().await.into_diagnostic()?;
        if !cmd.ignore_result && status.code() != Some(0) {
            return Err(crate::Error::TaskFailed {
                task_name,
                command: cmd.sh.clone(),
                exit_code: status.code(),
            }
            .into());
        }

        out_task.await.into_diagnostic()?;
        err_task.await.into_diagnostic()?;
    }

    Ok(())
}

const DEFAULT_TASK_NAME: &str = "default";

#[derive(Clone, Copy, PartialEq, Eq)]
enum TaskResult {
    Success,
    Failure,
}

#[async_trait::async_trait]
impl crate::commands::Command for Task {
    async fn run(
        &self,
        ctx: crate::commands::Context,
        config: crate::nurfile::NurFile,
    ) -> miette::Result<()> {
        let mut to_run = {
            let mut r = self.task_names.clone();
            if r.is_empty() {
                r.push(DEFAULT_TASK_NAME.to_string());
            }
            VecDeque::from(r)
        };

        // validate task names
        for task_name in &to_run {
            let task = config
                .tasks
                .get(task_name)
                .ok_or_else(|| crate::Error::NoSuchTask {
                    task_name: task_name.clone(),
                })?;

            for dep in &task.dependencies {
                if !config.tasks.contains_key(dep) {
                    return Err(crate::Error::NoSuchTask {
                        task_name: dep.clone(),
                    }
                    .into());
                }
            }
        }

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

        let mut shots = BTreeMap::new();
        for name in config.tasks.keys() {
            let (tx, rx) = oneshot::channel::<TaskResult>();
            shots.insert(name, (Some(tx), rx.shared()));
        }

        let mut seen = BTreeSet::new();
        let mut spawned: BTreeMap<String, JoinHandle<miette::Result<()>>> = BTreeMap::new();
        for task_name in &to_run {
            seen.insert(task_name.clone());
        }

        while let Some(task_name) = to_run.pop_front() {
            let task = config.tasks[&task_name].clone();

            let mut dep_shots = Vec::new();
            dep_shots.reserve(task.dependencies.len());
            for dep in &task.dependencies {
                if seen.insert(dep.clone()) {
                    to_run.push_back(dep.clone());
                }

                dep_shots.push(shots[&dep].1.clone());
            }
            let my_shot = shots.get_mut(&task_name).unwrap().0.take().unwrap();

            let ctx = ctx.clone();
            spawned.insert(
                task_name.clone(),
                tokio::spawn(async move {
                    for dep in dep_shots {
                        let tr = dep.await.into_diagnostic()?;
                        if tr == TaskResult::Failure {
                            // we don’t fail because upstream failed;
                            // produces better diagnostics.
                            return Ok(());
                        }
                    }

                    run_cmds(ctx, task_name, &task).await?;
                    let _ = my_shot.send(TaskResult::Success);
                    Ok(())
                }),
            );
        }

        for spawn in spawned.into_values() {
            spawn.await.into_diagnostic()??;
        }

        Ok(())
    }
}
