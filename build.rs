// build.rs — generates oms_generated.txt, OUT_DIR/codec.rs, OUT_DIR/subscribers.rs
//
// Reads the UCI XSD to discover enabled message types (filtered by RCAL_CALCONFIG_PATH),
// then emits COSMOS telemetry definitions and Rust encode/subscribe code for each
// enabled status message.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use quick_xml::events::Event;
use quick_xml::reader::Reader;

fn main() {
    println!("cargo:rerun-if-env-changed=RCAL_XSD_PATH");
    println!("cargo:rerun-if-env-changed=RCAL_CALCONFIG_PATH");
    println!("cargo:rerun-if-env-changed=RCAL_CALCONFIG_SERVICES");

    let xsd_path = find_xsd();
    println!("cargo:rerun-if-changed={}", xsd_path.display());
    if let Ok(p) = std::env::var("RCAL_CALCONFIG_PATH") {
        println!("cargo:rerun-if-changed={p}");
    }

    let schema = parse_xsd(&xsd_path);
    let enabled = enabled_elements();
    let mut specs = build_specs(&schema, &enabled);
    specs.sort_by(|a, b| a.msg_name.cmp(&b.msg_name));
    for (i, spec) in specs.iter_mut().enumerate() {
        spec.packet_id = (i + 1) as u16;
    }

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    generate_cosmos_config(&specs);
    generate_codec(&specs, &out_dir);
    generate_subscribers(&specs, &out_dir);
}

// ─── XSD location ────────────────────────────────────────────────────────────

fn find_xsd() -> PathBuf {
    if let Ok(p) = std::env::var("RCAL_XSD_PATH") {
        return PathBuf::from(p);
    }
    let dir = PathBuf::from("local_deps/rcal/schema");
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("UCI_MessageDefinitions_") && n.ends_with(".xsd"))
                    .unwrap_or(false)
            })
            .collect();
        files.sort_by(|a, b| b.cmp(a));
        if let Some(f) = files.first() {
            return f.clone();
        }
    }
    panic!("No XSD found; set RCAL_XSD_PATH");
}

// ─── XSD model ───────────────────────────────────────────────────────────────

struct Schema {
    elements: Vec<(String, String)>, // (element_name, type_local)
    complex_types: HashMap<String, Vec<FieldInfo>>, // type_name → fields
    simple_types: HashMap<String, Vec<String>>, // enum_name → XSD values in order
}

struct FieldInfo {
    name: String,
    type_local: String,
    min_occurs: u32,
    max_occurs: u32, // u32::MAX = unbounded
}

fn parse_xsd(path: &PathBuf) -> Schema {
    let content = std::fs::read_to_string(path).expect("read XSD");
    let mut reader = Reader::from_str(&content);
    reader.config_mut().trim_text(true);

    let mut schema = Schema {
        elements: Vec::new(),
        complex_types: HashMap::new(),
        simple_types: HashMap::new(),
    };

    let mut current_complex: Option<(String, Vec<FieldInfo>)> = None;
    let mut current_simple: Option<(String, Vec<String>)> = None;
    let mut in_string_restriction = false;
    // Deferred element — has annotation children, finalised on </element>
    let mut pending_element: Option<(String, String, u32, u32)> = None;

    loop {
        let ev = reader.read_event();
        let is_empty = matches!(ev, Ok(Event::Empty(_)));
        match ev {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                let local = local_name(e.name().as_ref());
                match local.as_str() {
                    "complexType" => {
                        if current_complex.is_none() {
                            if let Some(name) = attr(e, "name") {
                                current_complex = Some((name, Vec::new()));
                            }
                        }
                    }
                    "simpleType" => {
                        if current_simple.is_none() {
                            if let Some(name) = attr(e, "name") {
                                current_simple = Some((name, Vec::new()));
                                in_string_restriction = false;
                            }
                        }
                    }
                    "restriction" => {
                        if current_simple.is_some() {
                            let is_str = attr(e, "base")
                                .map(|b| b.ends_with("string"))
                                .unwrap_or(false);
                            if is_str {
                                in_string_restriction = true;
                            }
                        }
                    }
                    "enumeration" => {
                        // value attribute is on the start tag regardless of empty/non-empty
                        if in_string_restriction {
                            if let (Some(st), Some(val)) = (&mut current_simple, attr(e, "value")) {
                                st.1.push(val);
                            }
                        }
                    }
                    "element" => {
                        let name = attr(e, "name");
                        let type_ = attr(e, "type");
                        let min: u32 = attr(e, "minOccurs")
                            .and_then(|s| s.parse().ok())
                            .unwrap_or(1);
                        let max: u32 = match attr(e, "maxOccurs").as_deref() {
                            Some("unbounded") => u32::MAX,
                            Some(n) => n.parse().unwrap_or(1),
                            None => 1,
                        };
                        if let (Some(n), Some(t)) = (name, type_) {
                            let tl = local_part(&t);
                            if is_empty {
                                add_field(
                                    n,
                                    tl,
                                    min,
                                    max,
                                    &mut schema,
                                    &mut current_complex,
                                    &current_simple,
                                );
                            } else {
                                pending_element = Some((n, tl, min, max));
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let local = local_name(e.name().as_ref());
                match local.as_str() {
                    "element" => {
                        if let Some((n, tl, min, max)) = pending_element.take() {
                            add_field(
                                n,
                                tl,
                                min,
                                max,
                                &mut schema,
                                &mut current_complex,
                                &current_simple,
                            );
                        }
                    }
                    "complexType" => {
                        if let Some((name, fields)) = current_complex.take() {
                            schema.complex_types.insert(name, fields);
                        }
                    }
                    "simpleType" => {
                        if let Some((name, vals)) = current_simple.take() {
                            if !vals.is_empty() {
                                schema.simple_types.insert(name, vals);
                            }
                        }
                        in_string_restriction = false;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!("XSD parse error: {e}"),
            _ => {}
        }
    }

    schema
}

fn add_field(
    name: String,
    type_local: String,
    min: u32,
    max: u32,
    schema: &mut Schema,
    current_complex: &mut Option<(String, Vec<FieldInfo>)>,
    current_simple: &Option<(String, Vec<String>)>,
) {
    if let Some(ct) = current_complex {
        ct.1.push(FieldInfo {
            name,
            type_local,
            min_occurs: min,
            max_occurs: max,
        });
    } else if current_simple.is_none() {
        schema.elements.push((name, type_local));
    }
}

// ─── Calconfig subsetting ────────────────────────────────────────────────────

fn enabled_elements() -> Option<HashSet<String>> {
    let Ok(path) = std::env::var("RCAL_CALCONFIG_PATH") else {
        return None;
    };
    let service_filter: Option<HashSet<String>> =
        std::env::var("RCAL_CALCONFIG_SERVICES").ok().map(|s| {
            s.split(',')
                .map(|v| v.trim().to_owned())
                .filter(|v| !v.is_empty())
                .collect()
        });
    match calconfig_topics(&path, service_filter.as_ref()) {
        Ok(topics) => Some(topics),
        Err(e) => {
            println!("cargo:warning=Failed to parse RCAL_CALCONFIG_PATH={path}: {e}");
            None
        }
    }
}

// Mirrors rcal/build.rs calconfig_topics — kept in sync intentionally.
fn calconfig_topics(
    path: &str,
    service_filter: Option<&HashSet<String>>,
) -> Result<HashSet<String>, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("cannot read {path}: {e}"))?;
    let mut topics: HashSet<String> = HashSet::new();

    #[derive(Default)]
    enum Section {
        #[default]
        Other,
        Service,
        ServiceTopic,
    }

    let mut section = Section::default();
    let mut current_service_id = String::new();
    let mut current_topic_id = String::new();
    let mut current_topic_type: Option<String> = None;
    let mut in_selected_service = false;

    let flush = |id: &str, ty: Option<&str>, in_svc: bool, topics: &mut HashSet<String>| {
        if in_svc && !id.is_empty() {
            topics.insert(ty.unwrap_or(id).to_owned());
        }
    };

    for line in content.lines() {
        let t = line.trim();

        if t == "[[service]]" {
            flush(
                &current_topic_id,
                current_topic_type.as_deref(),
                in_selected_service,
                &mut topics,
            );
            current_topic_id.clear();
            current_topic_type = None;
            current_service_id.clear();
            in_selected_service = false;
            section = Section::Service;
            continue;
        }
        if t == "[[service.topic]]" {
            flush(
                &current_topic_id,
                current_topic_type.as_deref(),
                in_selected_service,
                &mut topics,
            );
            current_topic_id.clear();
            current_topic_type = None;
            section = Section::ServiceTopic;
            continue;
        }
        if t.starts_with("[[") {
            flush(
                &current_topic_id,
                current_topic_type.as_deref(),
                in_selected_service,
                &mut topics,
            );
            current_topic_id.clear();
            current_topic_type = None;
            section = Section::Other;
            continue;
        }

        match section {
            Section::Service => {
                if let Some(v) = parse_toml_str(t, "id") {
                    current_service_id = v;
                    in_selected_service = service_filter
                        .map(|f| f.contains(&current_service_id))
                        .unwrap_or(true);
                }
            }
            Section::ServiceTopic => {
                if let Some(v) = parse_toml_str(t, "id") {
                    current_topic_id = v;
                } else if let Some(v) = parse_toml_str(t, "type") {
                    current_topic_type = Some(v);
                }
            }
            Section::Other => {}
        }
    }

    flush(
        &current_topic_id,
        current_topic_type.as_deref(),
        in_selected_service,
        &mut topics,
    );
    Ok(topics)
}

fn parse_toml_str(line: &str, key: &str) -> Option<String> {
    let rest = line.strip_prefix(key)?.trim_start();
    let rest = rest.strip_prefix('=')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

// ─── Message spec ─────────────────────────────────────────────────────────────

struct MessageSpec {
    msg_name: String, // e.g. "SystemStatus"
    packet_id: u16,
    fields: Vec<FieldSpec>,
}

enum FieldSpec {
    Uuid {
        cosmos_name: String,
        accessor: String,
    },
    State {
        cosmos_name: String,
        accessor: String,
        enum_type: String,
        variants: Vec<String>,
    },
    Duration {
        cosmos_name: String,
        accessor: String,
    },
}

fn build_specs(schema: &Schema, enabled: &Option<HashSet<String>>) -> Vec<MessageSpec> {
    let mut specs = Vec::new();

    for (el_name, type_local) in &schema.elements {
        if !type_local.ends_with("MT") {
            continue;
        }
        if let Some(ref set) = enabled {
            if !set.contains(el_name) {
                continue;
            }
        }
        let Some(mt_fields) = schema.complex_types.get(type_local) else {
            continue;
        };
        let Some(mdt_field) = mt_fields.iter().find(|f| f.name == "MessageData") else {
            continue;
        };
        let mdt_type = mdt_field.type_local.clone();
        let Some(mdt_fields) = schema.complex_types.get(&mdt_type) else {
            continue;
        };

        let mut uuid_specs = Vec::new();
        let mut state_specs = Vec::new();
        let mut dur_specs = Vec::new();

        for f in mdt_fields {
            if f.min_occurs != 1 || f.max_occurs != 1 {
                continue;
            }
            if f.type_local.ends_with("ID_Type") {
                let upper = to_upper_snake(&f.name);
                let cosmos_name = if upper.ends_with("_ID") {
                    format!("{}_UUID", &upper[..upper.len() - 3])
                } else {
                    upper
                };
                uuid_specs.push(FieldSpec::Uuid {
                    cosmos_name,
                    accessor: snake(&f.name),
                });
            } else if f.name.contains("State") {
                if let Some(variants) = schema.simple_types.get(&f.type_local) {
                    state_specs.push(FieldSpec::State {
                        cosmos_name: to_upper_snake(&f.name),
                        accessor: snake(&f.name),
                        enum_type: f.type_local.clone(),
                        variants: variants.clone(),
                    });
                }
            } else if f.name == "TimeUp" && f.type_local == "DurationType" {
                dur_specs.push(FieldSpec::Duration {
                    cosmos_name: "TIME_UP".to_owned(),
                    accessor: "time_up".to_owned(),
                });
            }
        }

        let mut fields = uuid_specs;
        fields.extend(state_specs);
        fields.extend(dur_specs);

        if !fields.is_empty() {
            specs.push(MessageSpec {
                msg_name: el_name.clone(),
                packet_id: 0,
                fields,
            });
        }
    }

    specs
}

// ─── COSMOS config generator ──────────────────────────────────────────────────

fn generate_cosmos_config(specs: &[MessageSpec]) {
    use std::fmt::Write as _;
    let mut out = String::from("# @generated by build.rs — do not edit\n");

    for spec in specs {
        let pkt = to_upper_snake(&spec.msg_name);
        writeln!(
            out,
            "\nTELEMETRY OMS {pkt} BIG_ENDIAN \"OMS UCI {} message\"",
            spec.msg_name
        )
        .unwrap();
        writeln!(
            out,
            "  APPEND_ID_ITEM PACKET_ID 16 UINT {} \"Packet type identifier\"",
            spec.packet_id
        )
        .unwrap();
        writeln!(
            out,
            "  APPEND_ITEM TIMESTAMP_MS 64 INT \"Message timestamp (ms since Unix epoch)\""
        )
        .unwrap();
        writeln!(out, "    UNITS Milliseconds ms").unwrap();

        for field in &spec.fields {
            match field {
                FieldSpec::Uuid { cosmos_name, .. } => {
                    let prefix = cosmos_name
                        .strip_suffix("_UUID")
                        .map(upper_snake_to_title)
                        .unwrap_or_else(|| cosmos_name.clone());
                    writeln!(
                        out,
                        "  APPEND_ITEM {cosmos_name} 288 STRING \"{prefix} UUID (RFC 4122)\""
                    )
                    .unwrap();
                }
                FieldSpec::State {
                    cosmos_name,
                    enum_type,
                    variants,
                    ..
                } => {
                    writeln!(out, "  APPEND_ITEM {cosmos_name} 8 UINT \"{enum_type}\"").unwrap();
                    writeln!(out, "    STATE ENUM_NOT_SET 0").unwrap();
                    for (i, v) in variants.iter().enumerate() {
                        writeln!(out, "    STATE {v} {}", i + 1).unwrap();
                    }
                }
                FieldSpec::Duration { cosmos_name, .. } => {
                    writeln!(out, "  APPEND_ITEM {cosmos_name} 32 UINT \"Uptime\"").unwrap();
                    writeln!(out, "    UNITS Seconds s").unwrap();
                }
            }
        }
    }

    let path = PathBuf::from("targets/OMS/cmd_tlm/oms_generated.txt");
    std::fs::create_dir_all(path.parent().unwrap()).ok();
    std::fs::write(&path, &out).expect("write oms_generated.txt");
}

// ─── codec.rs generator ──────────────────────────────────────────────────────

fn generate_codec(specs: &[MessageSpec], out_dir: &std::path::Path) {
    use std::fmt::Write as _;
    let mut out = String::new();

    writeln!(out, "// @generated by build.rs — do not edit").unwrap();

    if specs.is_empty() {
        std::fs::write(out_dir.join("codec.rs"), out).expect("write codec.rs");
        std::fs::write(
            out_dir.join("subscribers.rs"),
            "// @generated — no messages enabled\n",
        )
        .expect("write subscribers.rs");
        return;
    }

    // Collect unique enum types, MT traits, and concrete types
    let mut enum_types: Vec<String> = Vec::new();
    let mut mt_traits: Vec<String> = Vec::new();
    let mut concrete_types: Vec<String> = Vec::new();
    for spec in specs {
        let mt = format!("{}MT", spec.msg_name);
        if !mt_traits.contains(&mt) {
            mt_traits.push(mt);
        }
        let concrete = format!("{}_", spec.msg_name);
        if !concrete_types.contains(&concrete) {
            concrete_types.push(concrete);
        }
        for f in &spec.fields {
            if let FieldSpec::State { enum_type, .. } = f {
                if !enum_types.contains(enum_type) {
                    enum_types.push(enum_type.clone());
                }
            }
        }
    }

    writeln!(
        out,
        "use rcal::uci::types::{{ID_Type, MessageType, {}}};",
        mt_traits.join(", ")
    )
    .unwrap();
    if !enum_types.is_empty() {
        writeln!(out, "use rcal::uci::types::{{{}}};", enum_types.join(", ")).unwrap();
    }
    writeln!(
        out,
        "use rcal::uci::types::{{{}}};",
        concrete_types.join(", ")
    )
    .unwrap();
    writeln!(out).unwrap();

    // Packet ID constants
    for spec in specs {
        let const_name = format!("PACKET_ID_{}", to_upper_snake(&spec.msg_name));
        writeln!(out, "pub const {const_name}: u16 = {};", spec.packet_id).unwrap();
    }
    writeln!(out).unwrap();

    // State conversion functions (one per unique enum type)
    let mut seen_state_fns: std::collections::HashSet<String> = std::collections::HashSet::new();
    for spec in specs {
        for field in &spec.fields {
            let FieldSpec::State {
                enum_type,
                variants,
                ..
            } = field
            else {
                continue;
            };
            let fn_name = format!("state_u8_{}", snake(enum_type));
            if !seen_state_fns.insert(fn_name.clone()) {
                continue;
            }
            writeln!(out, "fn {fn_name}(v: &{enum_type}) -> u8 {{").unwrap();
            writeln!(out, "    match v {{").unwrap();
            writeln!(out, "        {enum_type}::EnumNotSet => 0,").unwrap();
            for (i, xsd_val) in variants.iter().enumerate() {
                let variant = enum_variant(xsd_val);
                writeln!(out, "        {enum_type}::{variant} => {},", i + 1).unwrap();
            }
            writeln!(out, "    }}").unwrap();
            writeln!(out, "}}").unwrap();
            writeln!(out).unwrap();
        }
    }

    // uuid_str helper
    writeln!(
        out,
        r#"fn uuid_str(id: &dyn ID_Type) -> [u8; 36] {{
    let s = format!("{{}}", id.uuid());
    let mut buf = [0u8; 36];
    let bytes = s.as_bytes();
    buf[..bytes.len().min(36)].copy_from_slice(&bytes[..bytes.len().min(36)]);
    buf
}}"#
    )
    .unwrap();
    writeln!(out).unwrap();

    // frame helper
    writeln!(
        out,
        r#"fn frame(packet_id: u16, ts_ms: i64, fields: &[u8]) -> Vec<u8> {{
    let payload_len = 2 + 8 + fields.len();
    let mut out = Vec::with_capacity(4 + payload_len);
    out.extend_from_slice(&(payload_len as u32).to_be_bytes());
    out.extend_from_slice(&packet_id.to_be_bytes());
    out.extend_from_slice(&ts_ms.to_be_bytes());
    out.extend_from_slice(fields);
    out
}}"#
    )
    .unwrap();
    writeln!(out).unwrap();

    // Encode functions
    for spec in specs {
        let fn_name = format!("encode_{}", snake(&spec.msg_name));
        let concrete = format!("{}_", spec.msg_name);
        let const_name = format!("PACKET_ID_{}", to_upper_snake(&spec.msg_name));
        writeln!(out, "pub fn {fn_name}(msg: &{concrete}) -> Vec<u8> {{").unwrap();
        writeln!(
            out,
            "    let ts: rcal::xs::DateTime = *msg.message_header().timestamp();"
        )
        .unwrap();
        writeln!(
            out,
            "    let ts_ms = chrono::DateTime::<chrono::Utc>::from(ts).timestamp_millis();"
        )
        .unwrap();
        writeln!(out, "    let mut fields: Vec<u8> = Vec::new();").unwrap();

        for field in &spec.fields {
            match field {
                FieldSpec::Uuid { accessor, .. } => {
                    writeln!(
                        out,
                        "    fields.extend_from_slice(&uuid_str(msg.message_data().{accessor}()));"
                    )
                    .unwrap();
                }
                FieldSpec::State {
                    accessor,
                    enum_type,
                    ..
                } => {
                    let state_fn = format!("state_u8_{}", snake(enum_type));
                    writeln!(
                        out,
                        "    fields.push({state_fn}(msg.message_data().{accessor}()));"
                    )
                    .unwrap();
                }
                FieldSpec::Duration { accessor, .. } => {
                    writeln!(
                        out,
                        "    let time_up_ns: i64 = *msg.message_data().{accessor}();"
                    )
                    .unwrap();
                    writeln!(
                        out,
                        "    let time_up_secs = u32::try_from(time_up_ns.max(0) / 1_000_000_000).unwrap_or(u32::MAX);"
                    )
                    .unwrap();
                    writeln!(
                        out,
                        "    fields.extend_from_slice(&time_up_secs.to_be_bytes());"
                    )
                    .unwrap();
                }
            }
        }

        writeln!(out, "    frame({const_name}, ts_ms, &fields)").unwrap();
        writeln!(out, "}}").unwrap();
        writeln!(out).unwrap();
    }

    // Tests
    writeln!(out, "#[cfg(test)]").unwrap();
    writeln!(out, "mod tests {{").unwrap();
    writeln!(out, "    use super::*;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    #[test]").unwrap();
    writeln!(out, "    fn frame_has_correct_length_prefix() {{").unwrap();
    writeln!(out, "        let pkt = frame(1, 0, &[0u8; 1]);").unwrap();
    writeln!(
        out,
        "        let stored_len = u32::from_be_bytes(pkt[..4].try_into().unwrap());"
    )
    .unwrap();
    writeln!(out, "        // payload = 2 + 8 + 1 = 11").unwrap();
    writeln!(out, "        assert_eq!(stored_len, 11);").unwrap();
    writeln!(out, "        assert_eq!(pkt.len(), 15);").unwrap();
    writeln!(out, "    }}").unwrap();

    for spec in specs {
        for field in &spec.fields {
            let FieldSpec::State {
                enum_type,
                variants,
                ..
            } = field
            else {
                continue;
            };
            if variants.is_empty() {
                continue;
            }
            let fn_name = format!("state_u8_{}", snake(enum_type));
            let last_variant = enum_variant(variants.last().unwrap());
            let last_idx = variants.len();
            let test_name = format!("state_roundtrip_{}", snake(&spec.msg_name));
            writeln!(out).unwrap();
            writeln!(out, "    #[test]").unwrap();
            writeln!(out, "    fn {test_name}() {{").unwrap();
            writeln!(
                out,
                "        assert_eq!({fn_name}(&{enum_type}::EnumNotSet), 0);"
            )
            .unwrap();
            writeln!(
                out,
                "        assert_eq!({fn_name}(&{enum_type}::{last_variant}), {last_idx});"
            )
            .unwrap();
            writeln!(out, "    }}").unwrap();
        }
    }

    writeln!(out, "}}").unwrap();

    std::fs::write(out_dir.join("codec.rs"), out).expect("write codec.rs");
}

// ─── subscribers.rs generator ─────────────────────────────────────────────────

fn generate_subscribers(specs: &[MessageSpec], out_dir: &std::path::Path) {
    use std::fmt::Write as _;
    // include!() requires a single expression; wrap all blocks in one outer block.
    let mut out = String::from("// @generated by build.rs — do not edit\n{\n");

    for spec in specs {
        let concrete = format!("rcal::uci::types::{}_", spec.msg_name);
        let encode_fn = format!("crate::codec::encode_{}", snake(&spec.msg_name));
        let topic_name = &spec.msg_name;
        let err_msg = format!("{} reader failed", spec.msg_name);
        writeln!(out, "{{").unwrap();
        writeln!(out, "    let tx_clone = tx.clone();").unwrap();
        writeln!(
            out,
            "    if let Err(e) = svc.create_reader::<{concrete}, _>(",
        )
        .unwrap();
        writeln!(out, "        \"{topic_name}\",").unwrap();
        writeln!(out, "        rcal::cal::TopicQos::default(),").unwrap();
        writeln!(out, "        move |msg, _| {{").unwrap();
        writeln!(out, "            let pkt = {encode_fn}(msg);").unwrap();
        writeln!(
            out,
            "            let _ = tx_clone.send(::std::sync::Arc::new(pkt));"
        )
        .unwrap();
        writeln!(out, "        }},").unwrap();
        writeln!(out, "    ) {{").unwrap();
        writeln!(
            out,
            "        slog::error!(root_logger, \"{err_msg}\"; \"error\" => %e);"
        )
        .unwrap();
        writeln!(out, "    }}").unwrap();
        writeln!(out, "}}").unwrap();
    }

    out.push_str("}\n");
    std::fs::write(out_dir.join("subscribers.rs"), out).expect("write subscribers.rs");
}

// ─── Name helpers ─────────────────────────────────────────────────────────────

fn local_name(name: &[u8]) -> String {
    let s = std::str::from_utf8(name).unwrap_or("");
    s.rfind(':').map(|i| &s[i + 1..]).unwrap_or(s).to_string()
}

fn local_part(s: &str) -> String {
    s.rfind(':').map(|i| &s[i + 1..]).unwrap_or(s).to_string()
}

fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .filter_map(|a| a.ok())
        .find(|a| local_name(a.key.as_ref()) == key)
        .and_then(|a| a.unescape_value().ok().map(|s| s.into_owned()))
}

/// PascalCase / camelCase → snake_case (mirrors rcal/build.rs `snake`).
fn snake(s: &str) -> String {
    let mut out = String::new();
    let mut prev_upper = false;
    let mut prev_under = false;
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 && !prev_upper && !prev_under {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
            prev_upper = true;
            prev_under = false;
        } else if c == '_' {
            // rcal drops underscores from snake conversion of PascalCase names
            prev_under = true;
        } else {
            out.push(c);
            prev_upper = false;
            prev_under = false;
        }
    }
    out
}

/// XSD enumeration value → Rust variant name (mirrors rcal/build.rs `enum_variant`).
fn enum_variant(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) if first.is_ascii_digit() => {
            format!("V{first}{}", chars.as_str().to_lowercase())
        }
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
    }
}

/// PascalCase → UPPER_SNAKE (e.g. "SystemState" → "SYSTEM_STATE").
fn to_upper_snake(s: &str) -> String {
    let mut out = String::new();
    let mut prev_upper = false;
    let mut prev_under = false;
    for (i, c) in s.chars().enumerate() {
        if c == '_' {
            out.push('_');
            prev_upper = false;
            prev_under = true;
        } else if c.is_uppercase() {
            if i > 0 && !prev_upper && !prev_under {
                out.push('_');
            }
            out.push(c);
            prev_upper = true;
            prev_under = false;
        } else {
            out.push(c.to_ascii_uppercase());
            prev_upper = false;
            prev_under = false;
        }
    }
    out
}

/// "SERVICE" → "Service" (first letter upper, rest lower).
fn upper_snake_to_title(s: &str) -> String {
    let lower = s.to_ascii_lowercase();
    let mut chars = lower.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
    }
}
