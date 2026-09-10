use serde_json::{Value, json};

use super::*;

fn roundtrip(msg: &Message) -> Message {
    let value = serde_json::to_value(msg).expect("serialize");
    serde_json::from_value(value).expect("deserialize")
}

#[test]
fn request_envelope_roundtrip_omits_missing_arguments() {
    let msg = Message::Request(RequestMessage {
        seq: 1,
        command: "threads".into(),
        arguments: None,
    });
    let value = serde_json::to_value(&msg).unwrap();
    assert_eq!(
        value,
        json!({
            "type": "request",
            "seq": 1,
            "command": "threads",
        })
    );
    assert_eq!(roundtrip(&msg), msg);
}

#[test]
fn response_envelope_keeps_request_seq_underscore() {
    let msg = Message::Response(ResponseMessage {
        seq: 2,
        request_seq: 1,
        success: true,
        command: "initialize".into(),
        message: None,
        body: Some(json!({"supportsConfigurationDoneRequest": true})),
    });
    let value = serde_json::to_value(&msg).unwrap();
    assert!(value.get("requestSeq").is_none());
    assert_eq!(value["request_seq"], 1);
    assert_eq!(value["type"], "response");
    assert_eq!(roundtrip(&msg), msg);
}

#[test]
fn failed_response_roundtrip() {
    let msg = Message::Response(ResponseMessage {
        seq: 4,
        request_seq: 3,
        success: false,
        command: "stackTrace".into(),
        message: Some("notStopped".into()),
        body: Some(json!({
            "error": { "id": 1, "format": "not stopped" }
        })),
    });
    assert_eq!(roundtrip(&msg), msg);
}

#[test]
fn event_envelope_roundtrip() {
    let msg = Message::Event(EventMessage {
        seq: 5,
        event: "stopped".into(),
        body: Some(json!({ "reason": "entry", "threadId": 1 })),
    });
    let value = serde_json::to_value(&msg).unwrap();
    assert_eq!(value["type"], "event");
    assert_eq!(roundtrip(&msg), msg);
}

#[test]
fn initialize_request_matches_advertised_client_capabilities() {
    let msg = RequestMessage::new::<Initialize>(1, InitializeArguments::new("lldb-dap")).unwrap();
    assert_eq!(msg.seq, 1);
    assert_eq!(msg.command, "initialize");
    let args = msg.arguments.as_ref().expect("arguments");
    assert_eq!(args["adapterID"], "lldb-dap");
    assert_eq!(args["clientID"], "argus-tui");
    assert_eq!(args["clientName"], "argus-tui");
    assert_eq!(args["pathFormat"], "path");
    assert_eq!(args["linesStartAt1"], true);
    assert_eq!(args["columnsStartAt1"], true);
    assert_eq!(args["supportsVariableType"], true);
    assert!(args.get("supportsRunInTerminalRequest").is_none());
    assert!(args.get("supportsStartDebuggingRequest").is_none());

    let wrapped = Message::Request(msg);
    let value = serde_json::to_value(&wrapped).unwrap();
    assert_eq!(value["type"], "request");
}

#[test]
fn initialize_success_body_keeps_unknown_capability_flags() {
    let response = ResponseMessage {
        seq: 1,
        request_seq: 1,
        success: true,
        command: "initialize".into(),
        message: None,
        body: Some(json!({
            "supportsConfigurationDoneRequest": true,
            "supportsConditionalBreakpoints": true,
            "supportsFunctionBreakpoints": true,
            "exceptionBreakpointFilters": []
        })),
    };
    let caps = response.decode_success::<Initialize>().unwrap();
    assert!(caps.supports_configuration_done_request);
    assert!(caps.supports_conditional_breakpoints);
    assert_eq!(caps.extra["supportsFunctionBreakpoints"], true);
    assert_eq!(caps.extra["exceptionBreakpointFilters"], json!([]));
}

#[test]
fn initialize_omitted_body_defaults_capabilities() {
    let response = ResponseMessage {
        seq: 1,
        request_seq: 1,
        success: true,
        command: "initialize".into(),
        message: None,
        body: None,
    };
    let caps = response.decode_success::<Initialize>().unwrap();
    assert!(!caps.supports_configuration_done_request);
    assert!(caps.extra.is_empty());
}

#[test]
fn failed_response_is_protocol_error_not_decode_error() {
    let response = ResponseMessage {
        seq: 2,
        request_seq: 1,
        success: false,
        command: "initialize".into(),
        message: Some("cancelled".into()),
        body: Some(json!({
            "error": { "id": 9, "format": "cancelled by user", "showUser": true }
        })),
    };
    let err = response.decode_success::<Initialize>().unwrap_err();
    match err {
        ProtocolError::Failed {
            command,
            message,
            body,
        } => {
            assert_eq!(command, "initialize");
            assert_eq!(message.as_deref(), Some("cancelled"));
            let error = body.unwrap().error.unwrap();
            assert_eq!(error.id, 9);
            assert_eq!(error.format, "cancelled by user");
            assert_eq!(error.show_user, Some(true));
        }
        other => panic!("expected Failed, got {other:?}"),
    }
}

#[test]
fn unit_success_body_accepts_missing_null_and_empty_object() {
    for body in [None, Some(Value::Null), Some(json!({}))] {
        let response = ResponseMessage {
            seq: 3,
            request_seq: 2,
            success: true,
            command: "launch".into(),
            message: None,
            body,
        };
        response.decode_success::<Launch>().unwrap();
    }
}

#[test]
fn configuration_done_omits_arguments() {
    let msg = RequestMessage::new::<ConfigurationDone>(4, ()).unwrap();
    assert_eq!(msg.command, "configurationDone");
    assert!(msg.arguments.is_none());
}

#[test]
fn parse_initialized_event() {
    let event = EventMessage {
        seq: 2,
        event: "initialized".into(),
        body: None,
    };
    assert!(matches!(event.parse().unwrap(), Event::Initialized));
}

#[test]
fn parse_stopped_entry_event() {
    let event = EventMessage {
        seq: 8,
        event: "stopped".into(),
        body: Some(json!({
            "reason": "entry",
            "threadId": 1,
            "allThreadsStopped": true
        })),
    };
    match event.parse().unwrap() {
        Event::Stopped(stopped) => {
            assert_eq!(stopped.reason, StoppedReason::Entry);
            assert_eq!(stopped.thread_id, Some(1));
            assert_eq!(stopped.all_threads_stopped, Some(true));
        }
        other => panic!("expected Stopped, got {other:?}"),
    }
}

#[test]
fn parse_output_event() {
    let event = EventMessage {
        seq: 9,
        event: "output".into(),
        body: Some(json!({
            "category": "stdout",
            "output": "hello\n"
        })),
    };
    match event.parse().unwrap() {
        Event::Output(output) => {
            assert_eq!(output.category, Some(OutputCategory::Stdout));
            assert_eq!(output.output, "hello\n");
        }
        other => panic!("expected Output, got {other:?}"),
    }
}

#[test]
fn parse_unknown_event_is_not_an_error() {
    let event = EventMessage {
        seq: 10,
        event: "module".into(),
        body: Some(json!({ "reason": "new", "module": { "id": "a", "name": "a" } })),
    };
    match event.parse().unwrap() {
        Event::Unknown { event, body } => {
            assert_eq!(event, "module");
            assert!(body.is_some());
        }
        other => panic!("expected Unknown, got {other:?}"),
    }
}

#[test]
fn camel_case_ids_and_variable_type() {
    let args = serde_json::to_value(ContinueArguments {
        thread_id: 3,
        single_thread: None,
    })
    .unwrap();
    assert_eq!(args, json!({ "threadId": 3 }));

    let variable = Variable {
        name: "x".into(),
        value: "1".into(),
        type_field: Some("int".into()),
        variables_reference: 7,
        evaluate_name: None,
    };
    let value = serde_json::to_value(&variable).unwrap();
    assert_eq!(value["type"], "int");
    assert_eq!(value["variablesReference"], 7);
    assert!(value.get("type_field").is_none());
    assert_eq!(serde_json::from_value::<Variable>(value).unwrap(), variable);
}

#[test]
fn launch_arguments_serialize_program_and_keep_extra_keys() {
    let mut args = LaunchArguments::new("/tmp/hello");
    args.stop_on_entry = Some(true);
    let value = serde_json::to_value(&args).unwrap();
    assert_eq!(
        value,
        json!({
            "program": "/tmp/hello",
            "stopOnEntry": true,
        })
    );

    let parsed: LaunchArguments = serde_json::from_value(json!({
        "program": "/tmp/hello",
        "stopOnEntry": true,
        "initCommands": ["settings set target.x86-disassembly-flavor intel"]
    }))
    .unwrap();
    assert_eq!(parsed.program, "/tmp/hello");
    assert_eq!(parsed.stop_on_entry, Some(true));
    assert_eq!(
        parsed.extra["initCommands"],
        json!(["settings set target.x86-disassembly-flavor intel"])
    );
}

#[test]
fn disconnect_empty_arguments_are_omitted() {
    let msg = RequestMessage::new::<Disconnect>(11, DisconnectArguments::default()).unwrap();
    assert!(msg.arguments.is_none());
}

#[test]
fn stack_trace_arguments_use_thread_id() {
    let msg = RequestMessage::new::<StackTrace>(
        12,
        StackTraceArguments {
            thread_id: 1,
            start_frame: Some(0),
            levels: Some(20),
        },
    )
    .unwrap();
    assert_eq!(
        msg.arguments.unwrap(),
        json!({
            "threadId": 1,
            "startFrame": 0,
            "levels": 20,
        })
    );
}
