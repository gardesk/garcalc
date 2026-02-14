//! IPC server for garcalc daemon

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Result;
use garcalc_cas::Evaluator;
use garcalc_ipc::{Command, Mode, Response, ResponseData, socket_path};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::signal;

/// IPC server for garcalc
pub struct IpcServer {
    listener: UnixListener,
    running: Arc<AtomicBool>,
    visible: Arc<AtomicBool>,
    mode: Mode,
    evaluator: Evaluator,
    history_count: usize,
}

impl IpcServer {
    pub async fn new() -> Result<Self> {
        let socket = socket_path();

        // Remove stale socket
        if socket.exists() {
            std::fs::remove_file(&socket)?;
        }

        // Create parent directory if needed
        if let Some(parent) = socket.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let listener = UnixListener::bind(&socket)?;
        tracing::info!("IPC server listening on {}", socket.display());

        Ok(Self {
            listener,
            running: Arc::new(AtomicBool::new(true)),
            visible: Arc::new(AtomicBool::new(false)),
            mode: Mode::Calculator,
            evaluator: Evaluator::new(),
            history_count: 0,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let running = self.running.clone();

        // Handle shutdown signals
        let running_signal = running.clone();
        tokio::spawn(async move {
            let _ = signal::ctrl_c().await;
            running_signal.store(false, Ordering::SeqCst);
        });

        while running.load(Ordering::SeqCst) {
            tokio::select! {
                result = self.listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            self.handle_client(stream).await?;
                        }
                        Err(e) => {
                            tracing::error!("Failed to accept connection: {}", e);
                        }
                    }
                }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    // Periodic check for shutdown
                }
            }
        }

        // Cleanup
        let socket = socket_path();
        let _ = std::fs::remove_file(&socket);

        Ok(())
    }

    async fn handle_client(&mut self, stream: UnixStream) -> Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();

        while reader.read_line(&mut line).await? > 0 {
            let command: Command = match serde_json::from_str(line.trim()) {
                Ok(cmd) => cmd,
                Err(e) => {
                    let response = Response::err(format!("Invalid command: {e}"));
                    let json = serde_json::to_string(&response)?;
                    writer.write_all(json.as_bytes()).await?;
                    writer.write_all(b"\n").await?;
                    writer.flush().await?;
                    line.clear();
                    continue;
                }
            };

            let response = self.handle_command(command).await;
            let json = serde_json::to_string(&response)?;
            writer.write_all(json.as_bytes()).await?;
            writer.write_all(b"\n").await?;
            writer.flush().await?;

            line.clear();
        }

        Ok(())
    }

    async fn handle_command(&mut self, command: Command) -> Response {
        match command {
            Command::Show => {
                self.visible.store(true, Ordering::SeqCst);
                self.spawn_window(true);
                Response::ok()
            }
            Command::Hide => {
                self.visible.store(false, Ordering::SeqCst);
                Response::ok()
            }
            Command::Toggle => {
                let was_visible = self.visible.fetch_xor(true, Ordering::SeqCst);
                if !was_visible {
                    self.spawn_window(true);
                }
                Response::ok()
            }
            Command::Evaluate { expr } => match self.evaluate(&expr) {
                Ok((input, result, exact)) => {
                    self.history_count += 1;
                    Response::ok_with_data(ResponseData::Evaluation {
                        input,
                        result,
                        exact,
                    })
                }
                Err(e) => Response::err(e.to_string()),
            },
            Command::GetMode => Response::ok_with_data(ResponseData::Mode { mode: self.mode }),
            Command::SetMode { mode } => {
                self.mode = mode;
                Response::ok()
            }
            Command::OpenDocument { path: _ } => {
                Response::err("Document loading not yet implemented")
            }
            Command::Status => Response::ok_with_data(ResponseData::Status {
                visible: self.visible.load(Ordering::SeqCst),
                mode: self.mode,
                document: None,
                history_count: self.history_count,
            }),
            Command::Quit => {
                self.running.store(false, Ordering::SeqCst);
                Response::ok()
            }
        }
    }

    fn evaluate(&self, input: &str) -> Result<(String, String, Option<String>)> {
        let expr = garcalc_cas::parser::parse(input)?;
        let result = self.evaluator.eval(&expr)?;
        Ok((input.to_string(), result.to_string(), None))
    }

    fn spawn_window(&self, popup: bool) {
        let mode = format!("{:?}", self.mode).to_lowercase();
        let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
        cmd.arg("--mode").arg(&mode);
        if popup {
            cmd.arg("--popup");
        }
        let _ = cmd.spawn();
    }
}
