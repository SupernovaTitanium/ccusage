use std::{
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use crate::{Result, cli_error};

pub(super) fn logical_path(path: &Path) -> PathBuf {
    if path.to_string_lossy().ends_with(".jsonl.zst") {
        path.with_extension("")
    } else {
        path.to_path_buf()
    }
}

pub(super) fn open(path: &Path) -> Result<Box<dyn BufRead>> {
    let file =
        File::open(path).map_err(|error| cli_error(format!("{}: {error}", path.display())))?;
    if path.to_string_lossy().ends_with(".jsonl.zst") {
        let decoder = zstd::stream::read::Decoder::new(file)
            .map_err(|error| cli_error(format!("{}: zstd: {error}", path.display())))?;
        Ok(Box::new(BufReader::with_capacity(128 * 1024, decoder)))
    } else {
        Ok(Box::new(BufReader::with_capacity(128 * 1024, file)))
    }
}

pub(super) fn read_until(
    reader: &mut dyn BufRead,
    path: &Path,
    line: &mut Vec<u8>,
) -> Result<usize> {
    reader
        .read_until(b'\n', line)
        .map_err(|error| cli_error(format!("{}: {error}", path.display())))
}

#[cfg(test)]
mod tests {
    use crate::{
        PricingMap,
        cli::{AgentReportKind, CodexSpeed, SharedArgs},
    };
    use ccusage_test_support::fs_fixture;

    fn log(id: &str, parent: Option<&str>, values: &[u64]) -> String {
        let mut lines = vec![serde_json::json!({"timestamp":"2026-10-01T22:00:00Z", "type":"session_meta", "payload":{"id":id,"forked_from_id":parent}}).to_string()];
        for (index, input) in values.iter().enumerate() {
            lines.push(serde_json::json!({"timestamp":format!("2026-10-01T22:{index:02}:00Z"), "type":"event_msg", "payload":{"type":"token_count", "info":{"model":"gpt-5", "last_token_usage":{"input_tokens":input,"cached_input_tokens":10,"cache_write_input_tokens":5,"output_tokens":20,"reasoning_output_tokens":2,"total_tokens":input+20}}}}).to_string());
        }
        lines.join("\n")
    }

    #[test]
    fn compressed_daily_cost_and_parent_replay_equal_plain() {
        let fixture = fs_fixture!({"sessions/parent.jsonl": log("parent", None, &[100]),
            "sessions/child.jsonl": log("child", Some("parent"), &[100, 200])});
        let dir = fixture.path("sessions");
        let plain = crate::load_codex_events_from_directory(&dir, true).unwrap();
        assert_eq!(
            plain.iter().map(|event| event.input_tokens).sum::<u64>(),
            300
        );
        let prices = PricingMap::load_embedded();
        let before = crate::report_json(
            &plain,
            AgentReportKind::Daily,
            Some("Etc/GMT-3"),
            &prices,
            CodexSpeed::Standard,
        )
        .unwrap();
        for name in ["parent", "child"] {
            let path = dir.join(format!("{name}.jsonl"));
            let raw = std::fs::read(&path).unwrap();
            std::fs::write(
                path.with_extension("jsonl.zst"),
                zstd::stream::encode_all(raw.as_slice(), 1).unwrap(),
            )
            .unwrap();
        }
        let coexist = crate::load_codex_events_from_directory(&dir, false).unwrap();
        assert_eq!(plain, coexist);
        for name in ["parent", "child"] {
            std::fs::remove_file(dir.join(format!("{name}.jsonl"))).unwrap();
        }
        let compressed = crate::load_codex_events_from_directory(&dir, false).unwrap();
        assert_eq!(plain, compressed);
        assert_eq!(
            before,
            crate::report_json(
                &compressed,
                AgentReportKind::Daily,
                Some("Etc/GMT-3"),
                &prices,
                CodexSpeed::Standard
            )
            .unwrap()
        );
        assert!(
            super::super::aggregate::load_groups_from_directory(
                &dir,
                &SharedArgs {
                    offline: true,
                    ..SharedArgs::default()
                },
                AgentReportKind::Daily
            )
            .is_ok()
        );
    }

    #[test]
    fn corrupt_and_truncated_streams_abort_loader_and_aggregation() {
        for raw in [b"not zstd".to_vec(), {
            let mut raw =
                zstd::stream::encode_all(log("session", None, &[100]).as_bytes(), 1).unwrap();
            raw.pop();
            raw
        }] {
            let fixture = fs_fixture!({"sessions/session.jsonl.zst": ""});
            std::fs::write(fixture.path("sessions/session.jsonl.zst"), raw).unwrap();
            for single_thread in [true, false] {
                assert!(
                    crate::load_codex_events_from_directory(
                        &fixture.path("sessions"),
                        single_thread
                    )
                    .is_err()
                );
                assert!(
                    super::super::aggregate::load_groups_from_directory(
                        &fixture.path("sessions"),
                        &SharedArgs {
                            single_thread,
                            ..SharedArgs::default()
                        },
                        AgentReportKind::Daily
                    )
                    .is_err()
                );
            }
        }
    }
}
