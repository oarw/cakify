use cakify_core::{AppCommand, AppEvent, ConversationId, RequestId, RunId};

#[test]
fn public_ids_and_events_are_serializable() {
    let event = AppEvent::DraftAccepted {
        request_id: RequestId::new(1),
        conversation_id: ConversationId::new(2),
        run_id: RunId::new(3),
        revision: 4,
    };
    let json = serde_json::to_string(&event).expect("event JSON");
    assert!(json.contains("DraftAccepted"));

    let command = AppCommand::Bootstrap;
    assert_eq!(
        serde_json::to_string(&command).expect("command JSON"),
        "\"Bootstrap\""
    );
}
