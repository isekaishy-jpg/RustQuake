use qw_common::{InfoString, MAX_INFO_STRING, PROTOCOL_VERSION};

pub struct ClientConfig {
    pub userinfo: InfoString,
}

impl ClientConfig {
    pub fn default() -> Self {
        let mut info = InfoString::new(MAX_INFO_STRING);
        let _ = info.set("name", "unnamed");
        let _ = info.set("topcolor", "0");
        let _ = info.set("bottomcolor", "0");
        let _ = info.set("rate", "2500");
        let _ = info.set("msg", "1");
        let _ = info.set_star("*ver", &format!("rq{}", PROTOCOL_VERSION));
        Self { userinfo: info }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_default_userinfo() {
        let config = ClientConfig::default();
        let info = config.userinfo.as_str();
        assert!(info.contains("\\name\\unnamed"));
        assert!(info.contains("\\rate\\2500"));
        assert!(info.contains("\\*ver\\rq"));
    }
}
