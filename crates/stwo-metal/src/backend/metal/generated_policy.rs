use super::artifact::{
    MetalArtifactLookupError, MetalArtifactRegistry, MetalArtifactSupportError,
    MetalComponentArtifact, MetalGeneratedRouteKind,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum UnsupportedGeneratedComponentReason {
    ArtifactNotRegistered,
    SchemaMismatch { expected: u16, found: u16 },
    UnsupportedRoute(MetalGeneratedRouteKind),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct UnsupportedGeneratedComponent<'a> {
    pub component_name: &'a str,
    pub reason: UnsupportedGeneratedComponentReason,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum MetalGeneratedRoutePolicy<'a> {
    UseRegisteredArtifact(MetalComponentArtifact),
    Reject(UnsupportedGeneratedComponent<'a>),
}

pub(crate) fn resolve_generated_route_policy<'a>(
    registry: &MetalArtifactRegistry,
    expected_schema_version: u16,
    component_name: &'a str,
    route: MetalGeneratedRouteKind,
) -> MetalGeneratedRoutePolicy<'a> {
    match registry.artifact_supporting_generated_route(component_name, expected_schema_version, route) {
        Ok(artifact) => MetalGeneratedRoutePolicy::UseRegisteredArtifact(artifact),
        Err(MetalArtifactSupportError::Lookup(
            MetalArtifactLookupError::SchemaMismatch { expected, found },
        )) => MetalGeneratedRoutePolicy::Reject(UnsupportedGeneratedComponent {
            component_name,
            reason: UnsupportedGeneratedComponentReason::SchemaMismatch { expected, found },
        }),
        Err(MetalArtifactSupportError::Lookup(MetalArtifactLookupError::UnknownComponent {
            component_name,
        })) => MetalGeneratedRoutePolicy::Reject(UnsupportedGeneratedComponent {
            component_name,
            reason: UnsupportedGeneratedComponentReason::ArtifactNotRegistered,
        }),
        Err(MetalArtifactSupportError::UnsupportedGeneratedRoute { component_name, route }) => {
            MetalGeneratedRoutePolicy::Reject(UnsupportedGeneratedComponent {
                component_name,
                reason: UnsupportedGeneratedComponentReason::UnsupportedRoute(route),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_generated_route_policy, MetalGeneratedRoutePolicy,
        UnsupportedGeneratedComponent, UnsupportedGeneratedComponentReason,
    };
    use crate::backend::metal::artifact::{
        MetalGeneratedRouteKind, STWO_METAL_ARTIFACT_REGISTRY_V1,
        STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
    };

    #[test]
    fn generated_policy_uses_registered_artifact_for_supported_route() {
        let decision = resolve_generated_route_policy(
            &STWO_METAL_ARTIFACT_REGISTRY_V1,
            STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
            "fibonacci_example",
            MetalGeneratedRouteKind::BenchmarkProveVerify,
        );

        match decision {
            MetalGeneratedRoutePolicy::UseRegisteredArtifact(artifact) => {
                assert_eq!(artifact.component_name, "fibonacci_example");
            }
            MetalGeneratedRoutePolicy::Reject(problem) => {
                panic!("expected registered artifact, got rejection: {problem:?}");
            }
        }
    }

    #[test]
    fn generated_policy_rejects_missing_component_explicitly() {
        let decision = resolve_generated_route_policy(
            &STWO_METAL_ARTIFACT_REGISTRY_V1,
            STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
            "missing_example",
            MetalGeneratedRouteKind::RegisteredProve,
        );

        assert_eq!(
            decision,
            MetalGeneratedRoutePolicy::Reject(UnsupportedGeneratedComponent {
                component_name: "missing_example",
                reason: UnsupportedGeneratedComponentReason::ArtifactNotRegistered,
            })
        );
    }

    #[test]
    fn generated_policy_rejects_unsupported_route_explicitly() {
        let decision = resolve_generated_route_policy(
            &STWO_METAL_ARTIFACT_REGISTRY_V1,
            STWO_METAL_ARTIFACT_SCHEMA_VERSION_V1,
            "poseidon_example",
            MetalGeneratedRouteKind::BenchmarkProveVerify,
        );

        assert_eq!(
            decision,
            MetalGeneratedRoutePolicy::Reject(UnsupportedGeneratedComponent {
                component_name: "poseidon_example",
                reason: UnsupportedGeneratedComponentReason::UnsupportedRoute(
                    MetalGeneratedRouteKind::BenchmarkProveVerify,
                ),
            })
        );
    }
}
