use crate::modules::entry_logs::RecordEntry;
use anyhow::{Context, bail};
use chrono::DateTime;
use doorsys_protocol::Audit;

pub(super) fn decode_audit(topic: &str, payload: &[u8]) -> anyhow::Result<RecordEntry> {
    let mut parts = topic.split('/');
    let net_id = match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("doorsys"), Some("audit"), Some(id), None)
            if !id.is_empty() && !id.contains(['+', '#']) =>
        {
            id
        }
        _ => bail!("Invalid audit topic"),
    };
    let audit: Audit = postcard::from_bytes(payload).context("Invalid audit payload")?;
    // Validate untrusted timestamps without potentially panicking in SystemTime conversion.
    let seconds =
        i64::try_from(audit.timestamp.secs_since_epoch).context("Invalid audit timestamp")?;
    if audit.timestamp.nanos_since_epoch >= 1_000_000_000 {
        bail!("Invalid audit timestamp");
    }
    let event_date = DateTime::from_timestamp(seconds, audit.timestamp.nanos_since_epoch)
        .context("Invalid audit timestamp")?;
    Ok(RecordEntry {
        code: audit.code,
        code_type: audit.code_type.to_string(),
        net_id: net_id.into(),
        success: audit.success,
        event_date,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use doorsys_protocol::{CodeType, Timestamp};
    fn audit() -> Audit {
        Audit {
            timestamp: Timestamp {
                secs_since_epoch: 1_700_000_000,
                nanos_since_epoch: 0,
            },
            code: 123456,
            code_type: CodeType::Pin,
            success: true,
        }
    }
    #[test]
    fn validates_topic_payload_and_timestamp() {
        let payload = postcard::to_allocvec(&audit()).unwrap();
        let decoded = decode_audit("doorsys/audit/door-1", &payload).unwrap();
        assert_eq!(decoded.net_id, "door-1");
        assert_eq!(decoded.code_type, "pin");
        for topic in [
            "doorsys/audit",
            "doorsys/audit/",
            "other/audit/door",
            "doorsys/audit/a/b",
            "doorsys/audit/+",
        ] {
            assert!(decode_audit(topic, &payload).is_err());
        }
        assert!(decode_audit("doorsys/audit/door", &[255]).is_err());
        let mut invalid = audit();
        invalid.timestamp.secs_since_epoch = u64::MAX;
        assert!(
            decode_audit(
                "doorsys/audit/door",
                &postcard::to_allocvec(&invalid).unwrap()
            )
            .is_err()
        );
    }
}
