use std::fmt;

use qw_common::NetAddr;

pub const DEFAULT_SERVER_PORT: u16 = 27500;
pub const DEFAULT_QPORT: u16 = 27001;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ClientMode {
    QuakeWorld,
    SinglePlayer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientArgs {
    pub mode: ClientMode,
    pub server: Option<NetAddr>,
    pub qport: u16,
    pub name: Option<String>,
    pub topcolor: Option<String>,
    pub bottomcolor: Option<String>,
    pub rate: Option<String>,
    pub data_dir: Option<String>,
    pub download_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliAction {
    Run(ClientArgs),
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliError {
    MissingServer,
    MissingValue(String),
    InvalidValue(String),
    InvalidFlag(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::MissingServer => write!(f, "missing server address"),
            CliError::MissingValue(flag) => write!(f, "missing value for {}", flag),
            CliError::InvalidValue(value) => write!(f, "invalid value: {}", value),
            CliError::InvalidFlag(flag) => write!(f, "unknown flag: {}", flag),
        }
    }
}

pub fn usage() -> &'static str {
    "Usage: qw-client --connect <ip[:port]> [options]\n\
Options:\n\
  --mode <qw|sp>             Client mode (default qw)\n\
  -c, --connect <ip[:port]>  Server address (default port 27500)\n\
  --qport <port>             Client qport (default 27001)\n\
  --name <name>              Player name\n\
  --topcolor <0-13>          Top color\n\
  --bottomcolor <0-13>       Bottom color\n\
  --rate <value>             Rate (bytes/sec)\n\
  --data-dir <path>          Override data directory\n\
  --download-dir <path>      Override download directory\n\
  -h, --help                 Show this help\n"
}

pub fn parse_args<I, S>(args: I) -> Result<CliAction, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut iter = args.into_iter().map(Into::into).peekable();
    let mut mode = ClientMode::QuakeWorld;
    let mut server_input: Option<String> = None;
    let mut qport = DEFAULT_QPORT;
    let mut name = None;
    let mut topcolor = None;
    let mut bottomcolor = None;
    let mut rate = None;
    let mut data_dir = None;
    let mut download_dir = None;

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(CliAction::Help),
            "--mode" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::MissingValue(arg.clone()))?;
                mode = match value.to_ascii_lowercase().as_str() {
                    "qw" | "quakeworld" => ClientMode::QuakeWorld,
                    "sp" | "single" | "singleplayer" => ClientMode::SinglePlayer,
                    _ => return Err(CliError::InvalidValue(value)),
                };
            }
            "-c" | "--connect" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::MissingValue(arg.clone()))?;
                server_input = Some(value);
            }
            "--qport" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::MissingValue(arg.clone()))?;
                qport = value
                    .parse::<u16>()
                    .map_err(|_| CliError::InvalidValue(value))?;
            }
            "--name" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::MissingValue(arg.clone()))?;
                name = Some(value);
            }
            "--topcolor" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::MissingValue(arg.clone()))?;
                topcolor = Some(value);
            }
            "--bottomcolor" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::MissingValue(arg.clone()))?;
                bottomcolor = Some(value);
            }
            "--rate" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::MissingValue(arg.clone()))?;
                rate = Some(value);
            }
            "--data-dir" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::MissingValue(arg.clone()))?;
                data_dir = Some(value);
            }
            "--download-dir" => {
                let value = iter
                    .next()
                    .ok_or_else(|| CliError::MissingValue(arg.clone()))?;
                download_dir = Some(value);
            }
            _ if arg.starts_with('-') => return Err(CliError::InvalidFlag(arg)),
            _ => {
                if server_input.is_none() {
                    server_input = Some(arg);
                } else {
                    return Err(CliError::InvalidValue(arg));
                }
            }
        }
    }

    let server = match server_input {
        Some(value) => Some(
            NetAddr::parse(&value, DEFAULT_SERVER_PORT)
                .map_err(|_| CliError::InvalidValue(value))?,
        ),
        None => None,
    };
    if mode == ClientMode::QuakeWorld && server.is_none() {
        return Err(CliError::MissingServer);
    }

    Ok(CliAction::Run(ClientArgs {
        mode,
        server,
        qport,
        name,
        topcolor,
        bottomcolor,
        rate,
        data_dir,
        download_dir,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_connect_with_default_port() {
        let args = vec!["--connect".to_string(), "127.0.0.1".to_string()];
        let action = parse_args(args).unwrap();
        let CliAction::Run(parsed) = action else {
            panic!("expected run action");
        };
        let server = parsed.server.expect("server");
        assert_eq!(server.ip, [127, 0, 0, 1]);
        assert_eq!(server.port, DEFAULT_SERVER_PORT);
        assert_eq!(parsed.qport, DEFAULT_QPORT);
    }

    #[test]
    fn parses_connect_and_name() {
        let args = vec![
            "--connect".to_string(),
            "10.0.0.5:27501".to_string(),
            "--name".to_string(),
            "unit".to_string(),
        ];
        let action = parse_args(args).unwrap();
        let CliAction::Run(parsed) = action else {
            panic!("expected run action");
        };
        let server = parsed.server.expect("server");
        assert_eq!(server.port, 27501);
        assert_eq!(parsed.name.as_deref(), Some("unit"));
    }

    #[test]
    fn rejects_missing_server() {
        let err = parse_args(Vec::<String>::new()).unwrap_err();
        assert_eq!(err, CliError::MissingServer);
    }

    #[test]
    fn allows_singleplayer_without_server() {
        let args = vec!["--mode".to_string(), "sp".to_string()];
        let action = parse_args(args).unwrap();
        let CliAction::Run(parsed) = action else {
            panic!("expected run action");
        };
        assert_eq!(parsed.mode, ClientMode::SinglePlayer);
        assert!(parsed.server.is_none());
    }
}
