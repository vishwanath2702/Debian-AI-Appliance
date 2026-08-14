//! Private `lsblk` JSON representations used by storage inspection.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct LsblkOutput {
    pub blockdevices: Vec<LsblkDevice>,
}

#[derive(Debug, Deserialize)]
pub struct LsblkDevice {
    pub path: String,

    #[serde(rename = "type")]
    pub device_type: String,

    pub rm: bool,
    pub wwn: Option<String>,
    pub serial: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::LsblkOutput;

    #[test]
    fn parses_lsblk_storage_fields() {
        let output: LsblkOutput = serde_json::from_str(
            r#"{
        "blockdevices": [
            {
                "path": "/dev/nvme0n1",
                "type": "disk",
                "rm": false,
                "wwn": "eui.2c3ebffff000220b",
                "serial": "AA000000000000008715"
            },
            {
                "path": "/dev/sda",
                "type": "disk",
                "rm": true,
                "wwn": null,
                "serial": "E0D55E6B6466E78088300791"
            }
        ]
    }"#,
        )
        .expect("lsblk JSON should parse");
        assert_eq!(output.blockdevices.len(), 2);

        assert_eq!(output.blockdevices[0].path, "/dev/nvme0n1");
        assert_eq!(output.blockdevices[0].device_type, "disk");
        assert!(!output.blockdevices[0].rm);
        assert_eq!(
            output.blockdevices[0].wwn.as_deref(),
            Some("eui.2c3ebffff000220b")
        );
        assert_eq!(
            output.blockdevices[0].serial.as_deref(),
            Some("AA000000000000008715")
        );

        assert_eq!(output.blockdevices[1].path, "/dev/sda");
        assert_eq!(output.blockdevices[1].device_type, "disk");
        assert!(output.blockdevices[1].rm);
        assert_eq!(output.blockdevices[1].wwn, None);
        assert_eq!(
            output.blockdevices[1].serial.as_deref(),
            Some("E0D55E6B6466E78088300791")
        );
    }
}
