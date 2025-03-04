use anyhow::Result;
use tokio::{io::{AsyncBufReadExt, BufReader}, select, sync::mpsc};
use yazi_plugin::external::{self, ShellOpt};
use yazi_proxy::AppProxy;
use yazi_shared::Defer;

use super::ProcessOpOpen;
use crate::{TaskProg, BLOCKER};

pub struct Process {
	prog: mpsc::UnboundedSender<TaskProg>,
}

impl Process {
	pub fn new(prog: mpsc::UnboundedSender<TaskProg>) -> Self { Self { prog } }

	pub async fn open(&self, mut task: ProcessOpOpen) -> Result<()> {
		if task.block {
			return self.open_block(task).await;
		}

		if task.orphan {
			return self.open_orphan(task).await;
		}

		self.prog.send(TaskProg::New(task.id, 0))?;
		let mut child = external::shell(ShellOpt {
			cmd: task.cmd,
			args: task.args,
			piped: true,
			..Default::default()
		})?;

		let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
		let mut stderr = BufReader::new(child.stderr.take().unwrap()).lines();
		loop {
			select! {
				_ = task.cancel.closed() => {
					child.start_kill().ok();
					break;
				}
				Ok(Some(line)) = stdout.next_line() => {
					self.log(task.id, line)?;
				}
				Ok(Some(line)) = stderr.next_line() => {
					self.log(task.id, line)?;
				}
				Ok(status) = child.wait() => {
					self.log(task.id, match status.code() {
						Some(code) => format!("Exited with status code: {code}"),
						None => "Process terminated by signal".to_string(),
					})?;
					if !status.success() {
						return self.fail(task.id, "Process failed".to_string());
					}
					break;
				}
			}
		}

		self.prog.send(TaskProg::Adv(task.id, 1, 0))?;
		self.succ(task.id)
	}

	async fn open_block(&self, task: ProcessOpOpen) -> Result<()> {
		let _guard = BLOCKER.acquire().await.unwrap();
		let _defer = Defer::new(AppProxy::resume);
		AppProxy::stop().await;

		let (id, cmd) = (task.id, task.cmd.clone());
		let result = external::shell(task.into());
		if let Err(e) = result {
			AppProxy::notify_warn(&cmd.to_string_lossy(), &format!("Failed to spawn process: {e}"));
			return self.succ(id);
		}

		let status = result.unwrap().wait().await?;
		if !status.success() {
			let content = match status.code() {
				Some(code) => format!("Process exited with status code: {code}"),
				None => "Process terminated by signal".to_string(),
			};
			AppProxy::notify_warn(&cmd.to_string_lossy(), &content);
		}

		self.succ(id)
	}

	async fn open_orphan(&self, task: ProcessOpOpen) -> Result<()> {
		let id = task.id;
		match external::shell(task.into()) {
			Ok(_) => self.succ(id)?,
			Err(e) => {
				self.prog.send(TaskProg::New(id, 0))?;
				self.fail(id, format!("Failed to spawn process: {e}"))?;
			}
		}

		Ok(())
	}
}

impl Process {
	#[inline]
	fn succ(&self, id: usize) -> Result<()> { Ok(self.prog.send(TaskProg::Succ(id))?) }

	#[inline]
	fn fail(&self, id: usize, reason: String) -> Result<()> {
		Ok(self.prog.send(TaskProg::Fail(id, reason))?)
	}

	#[inline]
	fn log(&self, id: usize, line: String) -> Result<()> {
		Ok(self.prog.send(TaskProg::Log(id, line))?)
	}
}
