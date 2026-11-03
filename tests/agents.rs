use std::time::Duration;

use cognicloud_sdk::{
    ActionLogListOptions, AgentStatus, Client, CreateAgent, CreateSkill, Error, PublishAgent,
    RunErrorCode, RunListOptions, RunStatus, RunWaitOptions, UpdateAgent, UpdateSkill,
};
use serde_json::json;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const AGENT: &str = "4f0b7a52-2c1d-4c8e-9a0e-0d1c2b3a4f5e";
const RUN: &str = "9a8b7c6d-1e2f-4a3b-8c9d-0e1f2a3b4c5d";
const SKILL: &str = "11111111-2222-4333-8444-555555555555";

fn agent() -> serde_json::Value {
    json!({
        "id": AGENT,
        "name": "Researcher",
        "description": "",
        "endpointId": "e1",
        "instructions": "Answer briefly.",
        "tools": ["browser"],
        "databaseId": null,
        "maxSteps": 10,
        "timeoutSeconds": 300,
        "status": "active",
        "invokeUrl": "https://cognicloud.dev/api/v1/agents/agents/x/invoke",
        "credentialPrefix": "ccag_6163",
        "templateSlug": null,
        "skillIds": [SKILL],
        "visibility": "private",
        "listingSlug": null,
        "publishedVersion": null,
        "hasUnpublishedChanges": false,
        "createdAt": "2026-11-03T10:00:00Z"
    })
}

fn skill() -> serde_json::Value {
    json!({
        "id": SKILL,
        "name": "Cite",
        "description": "When answering",
        "body": "Cite sources.",
        "version": 2,
        "createdAt": "2026-11-03T10:00:00Z",
        "updatedAt": "2026-11-03T11:00:00Z"
    })
}

fn run(status: &str) -> serde_json::Value {
    let finished = status != "running";
    json!({
        "id": RUN,
        "agentId": AGENT,
        "status": status,
        "input": "What is KSeF?",
        "output": if status == "completed" { json!("An e-invoicing system.") } else { json!(null) },
        "steps": [{
            "number": 1,
            "thought": "I know this.",
            "tool": null,
            "summary": "final answer",
            "error": null
        }],
        "errorCode": if status == "failed" { json!("max_steps") } else { json!(null) },
        "errorMessage": null,
        "maxSteps": 10,
        "timeoutSeconds": 300,
        "createdAt": "2026-11-03T10:00:00Z",
        "finishedAt": if finished { json!("2026-11-03T10:00:05Z") } else { json!(null) }
    })
}

fn client(server: &MockServer) -> Client {
    Client::builder()
        .token("amadeus-token")
        .tenant("acme")
        .agents_base_url(format!("{}/v1/agents", server.uri()))
        .build()
        .unwrap()
}

#[tokio::test]
async fn creates_an_agent_with_the_session_and_tenant() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/agents"))
        .and(header("authorization", "Bearer amadeus-token"))
        .and(header("tenant_id", "acme"))
        .and(body_json(json!({
            "name": "Researcher",
            "endpointId": "e1",
            "instructions": "Answer briefly.",
            "tools": ["browser"],
            "maxSteps": 10,
            "timeoutSeconds": 300,
            "skillIds": [SKILL]
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "agent": agent(),
            "credential": { "secret": "ccag_6163_abc" }
        })))
        .expect(1)
        .mount(&server)
        .await;

    let created = client(&server)
        .agents()
        .create(&CreateAgent {
            name: "Researcher".to_owned(),
            endpoint_id: "e1".to_owned(),
            instructions: "Answer briefly.".to_owned(),
            tools: vec!["browser".to_owned()],
            max_steps: 10,
            timeout_seconds: 300,
            skill_ids: vec![SKILL.to_owned()],
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(created.agent.status, AgentStatus::Active);
    assert_eq!(created.credential.secret.expose(), "ccag_6163_abc");
    assert!(!format!("{:?}", created.credential).contains("abc"));
}

#[tokio::test]
async fn lists_reads_updates_rotates_and_deletes_agents() {
    let server = MockServer::start().await;
    let one = format!("/v1/agents/agents/{AGENT}");
    Mock::given(method("GET"))
        .and(path("/v1/agents/agents"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([agent()])))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(one.as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(agent()))
        .mount(&server)
        .await;
    let mut paused = agent();
    paused["status"] = json!("paused");
    Mock::given(method("PATCH"))
        .and(path(one.as_str()))
        .and(body_json(json!({ "status": "paused", "databaseId": null })))
        .respond_with(ResponseTemplate::new(200).set_body_json(paused))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("{one}/credential/rotate")))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "secret": "ccag_6163_new" })),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path(one.as_str()))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let api = client.agents();
    assert_eq!(api.list().await.unwrap()[0].skill_ids, vec![SKILL]);
    assert_eq!(api.get(AGENT).await.unwrap().max_steps, 10);
    let updated = api
        .update(
            AGENT,
            &UpdateAgent {
                status: Some(AgentStatus::Paused),
                database_id: Some(None),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.status, AgentStatus::Paused);
    assert_eq!(
        api.rotate_credential(AGENT).await.unwrap().secret.expose(),
        "ccag_6163_new"
    );
    api.delete(AGENT).await.unwrap();
}

#[tokio::test]
async fn publishing_is_planned_and_answers_503() {
    let server = MockServer::start().await;
    for action in ["publish", "unpublish"] {
        Mock::given(method("POST"))
            .and(path(format!("/v1/agents/agents/{AGENT}/{action}")))
            .respond_with(ResponseTemplate::new(503).set_body_json(json!({
                "error": "unavailable",
                "message": "Publishing is not available yet."
            })))
            .expect(1)
            .mount(&server)
            .await;
    }

    let client = client(&server);
    let api = client.agents();
    let error = api
        .publish(
            AGENT,
            &PublishAgent {
                category: "research".to_owned(),
                publisher_name: "Acme".to_owned(),
                language: "en".to_owned(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error.status(), 503);
    assert_eq!(api.unpublish(AGENT).await.unwrap_err().status(), 503);
}

#[tokio::test]
async fn manages_skills_and_reads_their_versions() {
    let server = MockServer::start().await;
    let one = format!("/v1/agents/skills/{SKILL}");
    Mock::given(method("GET"))
        .and(path("/v1/agents/skills"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([skill()])))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/agents/skills"))
        .and(body_json(json!({
            "name": "Cite",
            "description": "When answering",
            "body": "Cite sources."
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(skill()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(one.as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(skill()))
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path(one.as_str()))
        .and(body_json(json!({ "body": "Cite two sources." })))
        .respond_with(ResponseTemplate::new(200).set_body_json(skill()))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("{one}/versions")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "version": 1,
            "name": "Cite",
            "description": "When answering",
            "body": "Cite.",
            "createdAt": "2026-11-03T10:00:00Z"
        }])))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path(one.as_str()))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let api = client.skills();
    assert_eq!(api.list().await.unwrap()[0].version, 2);
    api.create(&CreateSkill {
        name: "Cite".to_owned(),
        description: "When answering".to_owned(),
        body: "Cite sources.".to_owned(),
    })
    .await
    .unwrap();
    assert_eq!(api.get(SKILL).await.unwrap().name, "Cite");
    api.update(
        SKILL,
        &UpdateSkill {
            body: Some("Cite two sources.".to_owned()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(api.versions(SKILL).await.unwrap()[0].body, "Cite.");
    api.delete(SKILL).await.unwrap();
}

#[tokio::test]
async fn lists_and_revokes_keys_and_pages_actions() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/agents/keys"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
            "id": "k1",
            "name": "ci",
            "prefix": "cga_6163",
            "createdAt": "2026-11-03T10:00:00Z",
            "lastUsedAt": null
        }])))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/agents/keys/k1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/agents/actions"))
        .and(query_param("limit", "1"))
        .and(query_param("cursor", "c1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [{
                "id": "a1",
                "action": "start_run",
                "subjectId": RUN,
                "startedAt": "2026-11-03T10:00:00Z",
                "finishedAt": "2026-11-03T10:00:00Z",
                "durationMs": 3,
                "outcome": "succeeded",
                "errorCode": null
            }],
            "nextCursor": "c2"
        })))
        .mount(&server)
        .await;

    let client = client(&server);
    let api = client.agents();
    assert_eq!(api.keys().list().await.unwrap()[0].prefix, "cga_6163");
    api.keys().revoke("k1").await.unwrap();
    let page = api
        .actions(ActionLogListOptions {
            limit: Some(1),
            cursor: Some("c1".to_owned()),
        })
        .await
        .unwrap();
    assert_eq!(page.items[0].subject_id.as_deref(), Some(RUN));
    assert_eq!(page.next_cursor.as_deref(), Some("c2"));
}

#[tokio::test]
async fn starts_lists_reads_and_stops_runs() {
    let server = MockServer::start().await;
    let runs = format!("/v1/agents/agents/{AGENT}/runs");
    Mock::given(method("POST"))
        .and(path(runs.as_str()))
        .and(body_json(json!({ "input": "What is KSeF?" })))
        .respond_with(ResponseTemplate::new(202).set_body_json(run("running")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(runs.as_str()))
        .and(query_param("limit", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "items": [run("failed")],
            "nextCursor": null
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/agents/runs/{RUN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(run("completed")))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/agents/runs/{RUN}/stop")))
        .respond_with(ResponseTemplate::new(200).set_body_json(run("stopped")))
        .expect(1)
        .mount(&server)
        .await;

    let client = client(&server);
    let api = client.runs();
    let started = api.start(AGENT, "What is KSeF?").await.unwrap();
    assert_eq!(started.status, RunStatus::Running);
    assert_eq!(started.steps[0].tool, None);
    let page = api
        .list(
            AGENT,
            RunListOptions {
                limit: Some(5),
                cursor: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(page.items[0].error_code, Some(RunErrorCode::MaxSteps));
    assert_eq!(page.next_cursor, None);
    let done = api.get(RUN).await.unwrap();
    assert_eq!(done.output.as_deref(), Some("An e-invoicing system."));
    assert_eq!(api.stop(RUN).await.unwrap().status, RunStatus::Stopped);
}

#[tokio::test]
async fn reads_statuses_and_error_codes_it_does_not_know() {
    let server = MockServer::start().await;
    let mut future = run("failed");
    future["status"] = json!("queued");
    future["errorCode"] = json!("budget_exceeded");
    Mock::given(method("GET"))
        .and(path(format!("/v1/agents/runs/{RUN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(future))
        .mount(&server)
        .await;

    let run = client(&server).runs().get(RUN).await.unwrap();
    assert_eq!(run.status, RunStatus::Unknown);
    assert_eq!(run.error_code, Some(RunErrorCode::Unknown));
}

#[tokio::test]
async fn a_run_start_is_not_retried_and_reports_the_service_code() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/agents/agents/{AGENT}/runs")))
        .respond_with(ResponseTemplate::new(429).set_body_json(json!({
            "error": "limit_reached",
            "message": "At most 3 runs at once."
        })))
        .expect(1)
        .mount(&server)
        .await;

    let error = client(&server).runs().start(AGENT, "Go").await.unwrap_err();
    assert_eq!(error.status(), 429);
    assert_eq!(error.code(), "limit_reached");
}

#[tokio::test]
async fn run_and_wait_polls_until_the_run_finishes() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/agents/agents/{AGENT}/runs")))
        .respond_with(ResponseTemplate::new(202).set_body_json(run("running")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/agents/runs/{RUN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(run("running")))
        .up_to_n_times(2)
        .expect(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/agents/runs/{RUN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(run("completed")))
        .expect(1)
        .mount(&server)
        .await;

    let finished = client(&server)
        .runs()
        .run_and_wait(
            AGENT,
            "What is KSeF?",
            RunWaitOptions {
                poll_interval: Duration::from_millis(10),
                timeout: Duration::from_secs(5),
            },
        )
        .await
        .unwrap();
    assert_eq!(finished.status, RunStatus::Completed);
}

#[tokio::test]
async fn run_and_wait_times_out_with_the_sdk_timeout() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/agents/agents/{AGENT}/runs")))
        .respond_with(ResponseTemplate::new(202).set_body_json(run("running")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/agents/runs/{RUN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(run("running")))
        .mount(&server)
        .await;

    let error = client(&server)
        .runs()
        .run_and_wait(
            AGENT,
            "Go",
            RunWaitOptions {
                poll_interval: Duration::from_millis(10),
                timeout: Duration::from_millis(50),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(error, Error::Timeout));
}

#[test]
fn run_wait_options_default_to_two_seconds_and_fifteen_minutes() {
    let options = RunWaitOptions::default();
    assert_eq!(options.poll_interval, Duration::from_secs(2));
    assert_eq!(options.timeout, Duration::from_secs(900));
}

#[tokio::test]
async fn run_and_wait_with_default_options_polls_after_two_seconds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/agents/agents/{AGENT}/runs")))
        .respond_with(ResponseTemplate::new(202).set_body_json(run("running")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/agents/runs/{RUN}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(run("completed")))
        .expect(1)
        .mount(&server)
        .await;

    let started = std::time::Instant::now();
    let finished = client(&server)
        .runs()
        .run_and_wait(AGENT, "Go", RunWaitOptions::default())
        .await
        .unwrap();
    assert_eq!(finished.status, RunStatus::Completed);
    assert!(started.elapsed() >= Duration::from_millis(1_900));
}

#[test]
fn an_unknown_agent_status_is_never_sent() {
    let update = UpdateAgent {
        status: Some(AgentStatus::Unknown),
        ..UpdateAgent::default()
    };
    let result = serde_json::to_value(&update);
    assert!(result.is_err(), "{result:?}");
}

#[tokio::test]
async fn an_agent_credential_runs_its_agent_without_a_tenant() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(format!("/v1/agents/agents/{AGENT}/runs")))
        .and(header("authorization", "Bearer ccag_6163_abc"))
        .respond_with(ResponseTemplate::new(202).set_body_json(run("running")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/agents/agents"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "error": "forbidden",
            "message": "An agent credential reaches only its agent's runs."
        })))
        .mount(&server)
        .await;

    let client = Client::builder()
        .token("ccag_6163_abc")
        .agents_base_url(format!("{}/v1/agents", server.uri()))
        .build()
        .unwrap();
    client.runs().start(AGENT, "Go").await.unwrap();
    let error = client.agents().list().await.unwrap_err();
    assert_eq!((error.status(), error.code()), (403, "forbidden"));
    let requests = server.received_requests().await.unwrap();
    assert!(requests
        .iter()
        .all(|request| request.headers.get("tenant_id").is_none()));
}

#[tokio::test]
async fn a_key_with_an_explicit_tenant_still_sends_it() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/agents/skills"))
        .and(header("authorization", "Bearer cga_6163_abc"))
        .and(header("tenant_id", "acme"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .expect(1)
        .mount(&server)
        .await;

    let client = Client::builder()
        .token("cga_6163_abc")
        .tenant("acme")
        .agents_base_url(format!("{}/v1/agents", server.uri()))
        .build()
        .unwrap();
    assert!(client.skills().list().await.unwrap().is_empty());
}
