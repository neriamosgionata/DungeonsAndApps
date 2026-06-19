// Ammo and thrown weapon decrement helpers.
use super::*;

pub fn infer_ammo_type(weapon_name: &str) -> Option<&'static str> {
    let w = weapon_name.to_lowercase();
    if w.contains("bow") && !w.contains("crossbow") {
        Some("Arrow")
    } else if w.contains("crossbow") {
        Some("Bolt")
    } else if w.contains("musket")
        || w.contains("pistol")
        || w.contains("firearm")
        || w.contains("gun")
        || w.contains("rifle")
    {
        Some("Bullet")
    } else if w.contains("sling") {
        Some("Sling Bullet")
    } else if w.contains("blowgun") {
        Some("Needle")
    } else {
        None
    }
}

pub async fn decrement_thrown_weapon(
    db: &mut sqlx::PgConnection,
    character_id: Uuid,
    weapon_name: &str,
) -> Result<Option<(String, i32)>, AppError> {
    let sheet_json: Option<serde_json::Value> =
        sqlx::query_scalar("select sheet from characters where id = $1")
            .bind(character_id)
            .fetch_optional(&mut *db)
            .await?;
    let mut sheet = sheet_json.unwrap_or_else(|| serde_json::json!({}));
    let equipment = match sheet.get_mut("equipment").and_then(|v| v.as_array_mut()) {
        Some(arr) => arr,
        None => return Ok(None),
    };
    let wname_lower = weapon_name.to_lowercase();
    let mut found = false;
    let mut remaining = 0;
    for item in equipment.iter_mut() {
        if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
            if name.to_lowercase() == wname_lower || name.to_lowercase().starts_with(&wname_lower) {
                let qty = item.get("qty").and_then(|v| v.as_i64()).unwrap_or(0);
                if qty > 0 {
                    let new_qty = qty - 1;
                    item["qty"] = serde_json::json!(new_qty);
                    remaining = new_qty as i32;
                    found = true;
                    break;
                }
            }
        }
    }
    if found {
        sqlx::query("update characters set sheet = $1 where id = $2")
            .bind(&sheet)
            .bind(character_id)
            .execute(db)
            .await?;
        Ok(Some((weapon_name.to_string(), remaining)))
    } else {
        Ok(None)
    }
}

pub async fn decrement_ammo(
    db: &mut sqlx::PgConnection,
    character_id: Uuid,
    weapon_name: &str,
) -> Result<Option<(String, i32)>, AppError> {
    let ammo_type = match infer_ammo_type(weapon_name) {
        Some(a) => a,
        None => return Ok(None),
    };
    let sheet_json: Option<serde_json::Value> =
        sqlx::query_scalar("select sheet from characters where id = $1")
            .bind(character_id)
            .fetch_optional(&mut *db)
            .await?;
    let mut sheet = sheet_json.unwrap_or_else(|| serde_json::json!({}));
    let equipment = match sheet.get_mut("equipment").and_then(|v| v.as_array_mut()) {
        Some(arr) => arr,
        None => {
            return Err(AppError::BadRequest(format!(
                "No {} ammunition remaining for {}",
                ammo_type, weapon_name
            )));
        }
    };
    let mut found = false;
    let mut remaining = 0;
    for item in equipment.iter_mut() {
        if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
            if name.to_lowercase().contains(&ammo_type.to_lowercase()) {
                let qty = item.get("qty").and_then(|v| v.as_i64()).unwrap_or(0);
                if qty > 0 {
                    let new_qty = qty - 1;
                    item["qty"] = serde_json::json!(new_qty);
                    remaining = new_qty as i32;
                    found = true;
                    break;
                }
            }
        }
    }
    if !found {
        return Err(AppError::BadRequest(format!(
            "No {} ammunition remaining for {}",
            ammo_type, weapon_name
        )));
    }
    sqlx::query("update characters set sheet = $1 where id = $2")
        .bind(&sheet)
        .bind(character_id)
        .execute(db)
        .await?;
    Ok(Some((ammo_type.to_string(), remaining)))
}
