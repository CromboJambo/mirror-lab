use mirror_kernel::{
    ChallengeMirror, CompressMirror, EmpathicMirror, EventStore, ExpandMirror, KernelRegistry,
    MirrorEvent,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut registry = KernelRegistry::new();
    registry.register(EmpathicMirror);
    registry.register(ChallengeMirror);
    registry.register(CompressMirror);
    registry.register(ExpandMirror);

    let store = EventStore::new(":memory:")?;
    let event = MirrorEvent {
        id: "00000000-0000-0000-0000-000000000000".to_string(),
        timestamp: 1,
        source: "smoke_test".to_string(),
        content: "mirror-kernel workspace binary smoke test".to_string(),
        content_hash: Some(
            "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
        ),
        meta: None,
    };
    store.append_event(&event)?;

    let events = store.get_events()?;
    let reflections = registry.dispatch(&events, &[]);

    println!("Registered kernels: {}", registry.list_kernels().join(", "));
    println!("Produced reflections: {}", reflections.len());

    Ok(())
}
