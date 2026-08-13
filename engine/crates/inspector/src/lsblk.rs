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
                        "path": "/dev/sdb",
                        "type": "disk",
                        "rm": false
                    },
                    {
                        "path": "/dev/sdc",
                        "type": "disk",
                        "rm": true
                    }
                ]
            }"#,
        )
        .expect("lsblk JSON should parse");

        assert_eq!(output.blockdevices.len(), 2);
        assert_eq!(output.blockdevices[0].path, "/dev/sdb");
        assert_eq!(output.blockdevices[0].device_type, "disk");
        assert!(!output.blockdevices[0].rm);

        assert_eq!(output.blockdevices[1].path, "/dev/sdc");
        assert!(output.blockdevices[1].rm);
    }
}
