use rcal::uci::types::{ID_Type, MessageType, ServiceStatusMT, SubsystemStatusMT, SystemStatusMT};
use rcal::uci::types::{ServiceStateEnum, SubsystemStateEnum, SystemStateEnum};
use rcal::uci::types::{ServiceStatus_, SubsystemStatus_, SystemStatus_};

pub const PACKET_ID_SYSTEM_STATUS: u16 = 1;
pub const PACKET_ID_SERVICE_STATUS: u16 = 2;
pub const PACKET_ID_SUBSYSTEM_STATUS: u16 = 3;

fn system_state_u8(s: &SystemStateEnum) -> u8 {
    match s {
        SystemStateEnum::EnumNotSet => 0,
        SystemStateEnum::Operational => 1,
        SystemStateEnum::Unavailable => 2,
        SystemStateEnum::Degraded => 3,
        SystemStateEnum::Failed => 4,
        SystemStateEnum::Inactive => 5,
    }
}

fn service_state_u8(s: &ServiceStateEnum) -> u8 {
    match s {
        ServiceStateEnum::EnumNotSet => 0,
        ServiceStateEnum::Initializing => 1,
        ServiceStateEnum::Normal => 2,
        ServiceStateEnum::Degraded => 3,
        ServiceStateEnum::Paused => 4,
        ServiceStateEnum::Inoperable => 5,
    }
}

fn subsystem_state_u8(s: &SubsystemStateEnum) -> u8 {
    match s {
        SubsystemStateEnum::EnumNotSet => 0,
        SubsystemStateEnum::Unknown => 1,
        SubsystemStateEnum::Not_installed => 2,
        SubsystemStateEnum::Off => 3,
        SubsystemStateEnum::Pre_initialization => 4,
        SubsystemStateEnum::Initialization => 5,
        SubsystemStateEnum::Standby => 6,
        SubsystemStateEnum::Operate => 7,
        SubsystemStateEnum::Operate_rx_only => 8,
        SubsystemStateEnum::Operate_tx_only => 9,
        SubsystemStateEnum::Maintenance => 10,
        SubsystemStateEnum::Calibration => 11,
        SubsystemStateEnum::Initiated_bit => 12,
        SubsystemStateEnum::Shutdown => 13,
        SubsystemStateEnum::Degraded => 14,
    }
}

fn uuid_str(id: &dyn ID_Type) -> [u8; 36] {
    let s = format!("{}", id.uuid());
    let mut buf = [0u8; 36];
    let bytes = s.as_bytes();
    buf[..bytes.len().min(36)].copy_from_slice(&bytes[..bytes.len().min(36)]);
    buf
}

fn frame(packet_id: u16, ts_ms: i64, fields: &[u8]) -> Vec<u8> {
    // payload = [2 PACKET_ID] [8 TIMESTAMP_MS] [fields]
    let payload_len = 2 + 8 + fields.len();
    let mut out = Vec::with_capacity(4 + payload_len);
    out.extend_from_slice(&(payload_len as u32).to_be_bytes());
    out.extend_from_slice(&packet_id.to_be_bytes());
    out.extend_from_slice(&ts_ms.to_be_bytes());
    out.extend_from_slice(fields);
    out
}

pub fn encode_system_status(msg: &SystemStatus_) -> Vec<u8> {
    let ts: rcal::xs::DateTime = *msg.message_header().timestamp();
    let ts_ms = chrono::DateTime::<chrono::Utc>::from(ts).timestamp_millis();
    let uuid = uuid_str(msg.message_data().system_id());
    let state = system_state_u8(msg.message_data().system_state());
    let mut fields = [0u8; 37];
    fields[..36].copy_from_slice(&uuid);
    fields[36] = state;
    frame(PACKET_ID_SYSTEM_STATUS, ts_ms, &fields)
}

pub fn encode_service_status(msg: &ServiceStatus_) -> Vec<u8> {
    let ts: rcal::xs::DateTime = *msg.message_header().timestamp();
    let ts_ms = chrono::DateTime::<chrono::Utc>::from(ts).timestamp_millis();
    let uuid = uuid_str(msg.message_data().service_id());
    let state = service_state_u8(msg.message_data().service_state());
    // xs::Duration = i64 nanoseconds; clamp to [0, u32::MAX seconds]
    let time_up_ns: i64 = *msg.message_data().time_up();
    let time_up_secs = u32::try_from(time_up_ns.max(0) / 1_000_000_000).unwrap_or(u32::MAX);
    let mut fields = [0u8; 41];
    fields[..36].copy_from_slice(&uuid);
    fields[36] = state;
    fields[37..41].copy_from_slice(&time_up_secs.to_be_bytes());
    frame(PACKET_ID_SERVICE_STATUS, ts_ms, &fields)
}

pub fn encode_subsystem_status(msg: &SubsystemStatus_) -> Vec<u8> {
    let ts: rcal::xs::DateTime = *msg.message_header().timestamp();
    let ts_ms = chrono::DateTime::<chrono::Utc>::from(ts).timestamp_millis();
    let uuid = uuid_str(msg.message_data().subsystem_id());
    let state = subsystem_state_u8(msg.message_data().subsystem_state());
    let mut fields = [0u8; 37];
    fields[..36].copy_from_slice(&uuid);
    fields[36] = state;
    frame(PACKET_ID_SUBSYSTEM_STATUS, ts_ms, &fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_has_correct_length_prefix() {
        let pkt = frame(1, 0, &[0u8; 37]);
        // payload = 2 + 8 + 37 = 47; 4-byte length prefix + 47 = 51 total
        let stored_len = u32::from_be_bytes(pkt[..4].try_into().unwrap());
        assert_eq!(stored_len, 47);
        assert_eq!(pkt.len(), 51);
    }

    #[test]
    fn system_state_roundtrip() {
        assert_eq!(system_state_u8(&SystemStateEnum::EnumNotSet), 0);
        assert_eq!(system_state_u8(&SystemStateEnum::Operational), 1);
        assert_eq!(system_state_u8(&SystemStateEnum::Inactive), 5);
    }

    #[test]
    fn service_state_roundtrip() {
        assert_eq!(service_state_u8(&ServiceStateEnum::EnumNotSet), 0);
        assert_eq!(service_state_u8(&ServiceStateEnum::Normal), 2);
        assert_eq!(service_state_u8(&ServiceStateEnum::Inoperable), 5);
    }

    #[test]
    fn subsystem_state_roundtrip() {
        assert_eq!(subsystem_state_u8(&SubsystemStateEnum::EnumNotSet), 0);
        assert_eq!(subsystem_state_u8(&SubsystemStateEnum::Operate), 7);
        assert_eq!(subsystem_state_u8(&SubsystemStateEnum::Degraded), 14);
    }
}
