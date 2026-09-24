use inferenced_core::{
    arbiter::Arbiter,
    lease::LeasePriority,
    model::{ModelDescriptor, ModelPlacementState},
    topology::{ComputePlane, ComputePlaneKind, HardwareTopology},
};
use std::path::PathBuf;

#[tokio::test]
async fn test_e2e_model_and_lease_pipeline() {
    let mut topo = HardwareTopology::default();
    topo.planes.push(ComputePlane {
        id: "e2e-gpu".into(),
        name: "E2E Integration GPU".into(),
        kind: ComputePlaneKind::DiscreteGpu,
        device_path: None,
        total_memory_bytes: 8 * 1024 * 1024 * 1024,
        available_memory_bytes: 8 * 1024 * 1024 * 1024,
        numa_node: None,
        supported_formats: vec!["GGUF".into()],
        is_triage_reserved: false,
        hardware_features: vec!["vram-managed".into()],
    });

    let arbiter = Arbiter::new(topo);

    // Register a model
    let desc = ModelDescriptor {
        id: "test-model:1b".into(),
        format: "GGUF".into(),
        path: PathBuf::from("/models/test.gguf"),
        estimated_memory_bytes: 2 * 1024 * 1024 * 1024,
        placement: ModelPlacementState::Dormant,
        resident_plane_id: None,
        last_accessed: chrono::Utc::now(),
        access_count: 0,
        preferred_plane: Some("e2e-gpu".into()),
    };
    arbiter.register_model(desc).await;

    let models = arbiter.list_models().await;
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].id, "test-model:1b");

    // Acquire lease for model
    let lease = arbiter
        .acquire_lease(
            LeasePriority::Interactive,
            models[0].estimated_memory_bytes,
            Some("e2e-gpu".into()),
            Some("e2e-client.service".into()),
            None,
        )
        .await
        .expect("Lease acquisition should succeed");

    assert_eq!(lease.plane_id, "e2e-gpu");
    assert_eq!(lease.allocated_memory_bytes, 2 * 1024 * 1024 * 1024);

    // Verify remaining memory
    let current_topo = arbiter.get_topology().await;
    assert_eq!(
        current_topo.planes[0].available_memory_bytes,
        6 * 1024 * 1024 * 1024
    );

    // Release lease
    arbiter.release_lease(lease.id).await.unwrap();
    let released_topo = arbiter.get_topology().await;
    assert_eq!(
        released_topo.planes[0].available_memory_bytes,
        8 * 1024 * 1024 * 1024
    );
}
