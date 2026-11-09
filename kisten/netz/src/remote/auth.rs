use super::*;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Pair {
    code: String,
    device_name: String,
}
pub(super) async fn pair(
    State(s): State<RemoteControl>,
    Json(p): Json<Pair>,
) -> Result<Json<Value>, StatusCode> {
    if p.device_name.trim().is_empty() || p.device_name.len() > 100 {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let mut a = s.0.auth.lock().unwrap();
    if a.attempts >= 10 || now() >= a.expires {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    a.attempts += 1;
    if hash(&p.code.trim().to_ascii_lowercase()) != hash(&a.code) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let token = random().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let id = random().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let expires = now() + 86400 * 30;
    a.tokens.retain(|_, d| d.info.expires_at > now());
    if a.tokens.len() >= 64 {
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }
    a.tokens.insert(
        hash(&token),
        Device {
            info: RemoteDevice {
                device_id: id.clone(),
                device_name: p.device_name,
                expires_at: expires,
            },
        },
    );
    if s.save("auth", &*a).is_err() {
        a.tokens.remove(&hash(&token));
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    a.expires = 0;
    Ok(Json(
        json!({"token":token,"device_id":id,"expires_at":expires,"host_id":s.0.host_id}),
    ))
}
pub(super) async fn revoke(
    State(s): State<RemoteControl>,
    h: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    s.authorize(&h)?;
    let token = h["authorization"]
        .to_str()
        .unwrap()
        .strip_prefix("Bearer ")
        .unwrap();
    let mut a = s.0.auth.lock().unwrap();
    let previous = a.clone();
    a.tokens.remove(&hash(token));
    if s.save("auth", &*a).is_err() {
        *a = previous;
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    drop(a);
    s.0.terminals.revoke(&hash(token));
    Ok(StatusCode::NO_CONTENT)
}
