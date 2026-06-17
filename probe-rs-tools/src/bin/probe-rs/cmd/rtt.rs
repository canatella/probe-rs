use std::path::Path;
use time::UtcOffset;

use crate::rpc::client::RpcClient;
use crate::rpc::functions::monitor::{MonitorMode, MonitorOptions};
use crate::rpc::functions::rtt_client::ScanRegion;
use crate::util::cli::{self, connect_target_output_files, parse_semihosting_options, rtt_client};
use crate::util::common_options::ProbeOptions;

/// Stream RTT logs from a running target (no ELF required).
///
/// Scans target RAM for the RTT control block and prints the log channels.
/// Unlike `attach`, this attaches to the already-running core WITHOUT catching
/// the reset or hardfault vectors, so the target is never halted and keeps
/// running across reboots. No firmware/ELF argument is needed: the control
/// block is always located by scanning memory.
#[derive(clap::Parser)]
pub struct Cmd {
    #[clap(flatten)]
    pub(crate) probe_options: ProbeOptions,

    /// The format string to use when printing defmt encoded log messages.
    ///
    /// See <https://defmt.ferrous-systems.com/custom-log-output>
    #[clap(long)]
    pub(crate) log_format: Option<String>,

    /// Suppress filename and line number information from the rtt log.
    #[clap(long)]
    pub(crate) no_location: bool,

    /// Suppress timestamps from the rtt log.
    #[clap(long)]
    pub(crate) no_timestamps: bool,

    /// File name to store formatted output at (see `run --target-output-file`).
    #[clap(long)]
    pub(crate) target_output_file: Vec<String>,
}

impl Cmd {
    pub async fn run(self, client: RpcClient, utc_offset: UtcOffset) -> anyhow::Result<()> {
        // No ELF: always scan RAM for the control block. An empty "firmware"
        // (/dev/null) makes rtt_client skip symbol/defmt extraction and fall
        // back to the scan region we pass.
        let path = Path::new("/dev/null");

        let session = cli::attach_probe(&client, self.probe_options, true).await?;

        let rtt_client = rtt_client(
            &session,
            path,
            ScanRegion::TargetDefault,
            self.log_format,
            !self.no_timestamps,
            !self.no_location,
            Some(utc_offset),
        )
        .await?;

        let mut target_output_files = connect_target_output_files(self.target_output_file).await?;

        let client_handle = rtt_client.handle();

        cli::monitor(
            &session,
            MonitorMode::AttachToRunning,
            path,
            Some(rtt_client),
            MonitorOptions {
                // Do not catch reset/hardfault: never halt the target, so it
                // keeps running (and rebooting) while we stream its logs.
                catch_reset: false,
                catch_hardfault: false,
                rtt_client: Some(client_handle),
                semihosting_options: parse_semihosting_options(vec![])?,
            },
            false,
            &mut target_output_files,
            0,
        )
        .await?;

        Ok(())
    }
}
