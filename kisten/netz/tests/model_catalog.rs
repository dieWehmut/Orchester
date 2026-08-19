use orchester_laufzeit::harness::service::{
    SelfAgentActiveModel, SelfAgentModelCatalog, SelfAgentModelChoice, SelfAgentProviderChoice,
    SelfAgentProviderState,
};
use orchester_netz::{model_catalog_response, ActiveModelDto, ProviderChoiceStateDto};

fn choice(profile: Option<&str>, provider: &str, model: &str) -> SelfAgentModelChoice {
    SelfAgentModelChoice {
        profile: profile.map(str::to_owned),
        provider: provider.to_owned(),
        provider_name: format!("{provider} display"),
        model: model.to_owned(),
        reasoning_effort: Some("high".to_owned()),
        plan_reasoning_effort: None,
        service_tier: Some("priority".to_owned()),
    }
}

#[test]
fn model_catalog_projects_choices_without_provider_endpoints() {
    let catalog = SelfAgentModelCatalog {
        active: SelfAgentActiveModel::Configured(choice(None, "Relay", "gpt-active")),
        profiles: vec![choice(Some("review"), "OpenAI", "gpt-review")],
        providers: vec![
            SelfAgentProviderChoice {
                provider: "OpenAI".to_owned(),
                provider_name: "OpenAI".to_owned(),
                state: SelfAgentProviderState::Selectable {
                    model: "gpt-active".to_owned(),
                    wire_api: "responses".to_owned(),
                },
            },
            SelfAgentProviderChoice {
                provider: "Relay".to_owned(),
                provider_name: "Relay display".to_owned(),
                state: SelfAgentProviderState::Unavailable {
                    path: "model_providers.Relay.base_url".to_owned(),
                    message: "provider base URL is not configured".to_owned(),
                },
            },
        ],
        selected_provider: Some("Relay".to_owned()),
    };

    let dto = model_catalog_response(&catalog);

    assert_eq!(dto.schema_version, 1);
    assert!(matches!(
        dto.active,
        ActiveModelDto::Configured { ref choice } if choice.model == "gpt-active"
    ));
    assert!(!dto.providers[0].active);
    assert!(dto.providers[1].active);
    assert_eq!(dto.providers[0].state, ProviderChoiceStateDto::Selectable);
    assert_eq!(dto.providers[1].state, ProviderChoiceStateDto::Unavailable);
    assert_eq!(dto.profiles[0].profile, "review");

    let json = serde_json::to_string(&dto).expect("model catalog JSON");
    assert!(!json.contains("https://"));
    assert!(!json.contains("credential"));
}

#[test]
fn unresolved_active_models_preserve_only_validation_metadata() {
    let catalog = SelfAgentModelCatalog {
        active: SelfAgentActiveModel::Unresolved {
            path: "model_provider".to_owned(),
            message: "configured provider is unavailable".to_owned(),
        },
        profiles: Vec::new(),
        providers: Vec::new(),
        selected_provider: None,
    };

    let dto = model_catalog_response(&catalog);

    assert!(matches!(
        dto.active,
        ActiveModelDto::Unresolved { ref field, ref reason }
            if field == "model_provider" && reason == "configured provider is unavailable"
    ));
}
