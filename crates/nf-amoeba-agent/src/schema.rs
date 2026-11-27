//! A small structural check of the tool schemas (JSON Schema draft 2020-12 subset used by tools.rs).

use serde_json::Value;

const TYPES: [&str; 7] = [
    "object", "array", "string", "integer", "number", "boolean", "null",
];

/// Checks one schema node recursively. Returns the path of the first problem.
pub fn check_node(node: &Value, path: &str) -> Result<(), String> {
    let obj = node
        .as_object()
        .ok_or_else(|| format!("{path}: schema must be an object"))?;
    match obj.get("type") {
        None => {}
        Some(Value::String(t)) if TYPES.contains(&t.as_str()) => {}
        Some(Value::Array(ts))
            if !ts.is_empty()
                && ts
                    .iter()
                    .all(|t| t.as_str().is_some_and(|t| TYPES.contains(&t))) => {}
        Some(other) => return Err(format!("{path}: bad type {other}")),
    }
    if let Some(e) = obj.get("enum")
        && e.as_array().is_none_or(|a| a.is_empty())
    {
        return Err(format!("{path}: enum must be a non-empty array"));
    }
    if let Some(p) = obj.get("properties") {
        let props = p
            .as_object()
            .ok_or_else(|| format!("{path}: properties must be an object"))?;
        for (k, v) in props {
            check_node(v, &format!("{path}/properties/{k}"))?;
        }
        if let Some(req) = obj.get("required") {
            let req = req
                .as_array()
                .ok_or_else(|| format!("{path}: required must be an array"))?;
            for r in req {
                let name = r
                    .as_str()
                    .ok_or_else(|| format!("{path}: required names must be strings"))?;
                if !props.contains_key(name) {
                    return Err(format!("{path}: required '{name}' is not a property"));
                }
            }
        }
    }
    if let Some(ap) = obj.get("additionalProperties") {
        match ap {
            Value::Bool(_) => {}
            other => check_node(other, &format!("{path}/additionalProperties"))?,
        }
    }
    if let Some(items) = obj.get("items") {
        check_node(items, &format!("{path}/items"))?;
    }
    Ok(())
}

/// Checks one tool definition: name, description and an object-typed parameter schema.
pub fn check_definition(def: &Value) -> Result<(), String> {
    let name = def["name"].as_str().ok_or("definition without a name")?;
    if !name.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
        return Err(format!("{name}: name must be lower snake case"));
    }
    if def["description"].as_str().is_none_or(str::is_empty) {
        return Err(format!("{name}: missing description"));
    }
    let params = &def["parameters"];
    if params["type"] != "object" {
        return Err(format!("{name}: parameters must be an object schema"));
    }
    check_node(params, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn bad_required_name_is_refused() {
        let s = json!({"type":"object","properties":{"a":{"type":"string"}},"required":["b"]});
        assert!(check_node(&s, "t").is_err());
    }

    #[test]
    fn bad_type_is_refused() {
        assert!(check_node(&json!({"type":"float"}), "t").is_err());
    }

    #[test]
    fn non_object_parameters_are_refused() {
        let d = json!({"name":"x","description":"d","parameters":{"type":"string"}});
        assert!(check_definition(&d).is_err());
    }
}

/// Validates a value against a schema node of the subset above. Used for every tool call before anything is sent.
pub fn validate_value(schema: &Value, v: &Value, path: &str) -> Result<(), String> {
    let obj = schema
        .as_object()
        .ok_or_else(|| format!("{path}: bad schema"))?;
    if let Some(t) = obj.get("type") {
        let names: Vec<&str> = match t {
            Value::String(s) => vec![s.as_str()],
            Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
            _ => vec![],
        };
        if !names.iter().any(|n| type_matches(n, v)) {
            return Err(format!("{path}: expected {}", names.join(" or ")));
        }
    }
    if let Some(e) = obj.get("enum").and_then(Value::as_array)
        && !e.contains(v)
    {
        return Err(format!("{path}: not one of the allowed values"));
    }
    if let Some(n) = v.as_f64() {
        if let Some(m) = obj.get("minimum").and_then(Value::as_f64)
            && n < m
        {
            return Err(format!("{path}: must be at least {m}"));
        }
        if let Some(m) = obj.get("exclusiveMinimum").and_then(Value::as_f64)
            && n <= m
        {
            return Err(format!("{path}: must be greater than {m}"));
        }
    }
    if let Some(s) = v.as_str() {
        let len = s.chars().count() as u64;
        if obj
            .get("minLength")
            .and_then(Value::as_u64)
            .is_some_and(|m| len < m)
        {
            return Err(format!("{path}: too short"));
        }
        if obj
            .get("maxLength")
            .and_then(Value::as_u64)
            .is_some_and(|m| len > m)
        {
            return Err(format!("{path}: too long"));
        }
    }
    if let Some(a) = v.as_array() {
        if obj
            .get("minItems")
            .and_then(Value::as_u64)
            .is_some_and(|m| (a.len() as u64) < m)
        {
            return Err(format!("{path}: too few items"));
        }
        if obj
            .get("maxItems")
            .and_then(Value::as_u64)
            .is_some_and(|m| (a.len() as u64) > m)
        {
            return Err(format!("{path}: too many items"));
        }
        if let Some(items) = obj.get("items") {
            for (i, x) in a.iter().enumerate() {
                validate_value(items, x, &format!("{path}/{i}"))?;
            }
        }
    }
    if let Some(m) = v.as_object() {
        let props = obj.get("properties").and_then(Value::as_object);
        for (k, x) in m {
            match props.and_then(|p| p.get(k)) {
                Some(sub) => validate_value(sub, x, &format!("{path}/{k}"))?,
                None => match obj.get("additionalProperties") {
                    Some(Value::Bool(false)) | None => {
                        return Err(format!("{path}: unknown name '{k}'"));
                    }
                    Some(Value::Bool(true)) => {}
                    Some(sub) => validate_value(sub, x, &format!("{path}/{k}"))?,
                },
            }
        }
        if let Some(req) = obj.get("required").and_then(Value::as_array) {
            for r in req.iter().filter_map(Value::as_str) {
                if !m.contains_key(r) {
                    return Err(format!("{path}: '{r}' is required"));
                }
            }
        }
    }
    Ok(())
}

fn type_matches(t: &str, v: &Value) -> bool {
    match t {
        "object" => v.is_object(),
        "array" => v.is_array(),
        "string" => v.is_string(),
        "integer" => v.is_i64() || v.is_u64(),
        "number" => v.is_number(),
        "boolean" => v.is_boolean(),
        "null" => v.is_null(),
        _ => false,
    }
}

#[cfg(test)]
mod value_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn unknown_and_missing_names_are_refused() {
        let s = json!({"type":"object","properties":{"job":{"type":"integer","minimum":1}},"required":["job"],"additionalProperties":false});
        assert!(validate_value(&s, &json!({"job":3}), "a").is_ok());
        assert!(validate_value(&s, &json!({"job":3,"x":1}), "a").is_err());
        assert!(validate_value(&s, &json!({}), "a").is_err());
        assert!(validate_value(&s, &json!({"job":0}), "a").is_err());
        assert!(validate_value(&s, &json!({"job":1.5}), "a").is_err());
    }

    #[test]
    fn params_values_are_typed() {
        let s =
            json!({"type":"object","additionalProperties":{"type":["number","string","boolean"]}});
        assert!(validate_value(&s, &json!({"r1":200,"name":"x","ok":true}), "p").is_ok());
        assert!(validate_value(&s, &json!({"r1":[1]}), "p").is_err());
    }
}
