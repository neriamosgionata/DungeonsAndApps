// server-authoritative dice roller.
// syntax: NdS[+/-M][kh/kl/dh/dl N] — subset supporting adv/disadv via kh1/kl1.

use rand::Rng;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct RollTerm {
    pub expr: String,
    pub kind: TermKind,
    pub rolls: Vec<i32>,
    pub kept: Vec<i32>,
    pub value: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TermKind {
    Dice,
    Modifier,
}

#[derive(Debug, Clone, Serialize)]
pub struct RollResult {
    pub expression: String,
    pub terms: Vec<RollTerm>,
    pub total: i32,
}

#[derive(Debug, thiserror::Error)]
pub enum DiceError {
    #[error("invalid expression: {0}")]
    Invalid(String),
}

pub fn roll<R: Rng + ?Sized>(expression: &str, rng: &mut R) -> Result<RollResult, DiceError> {
    let cleaned: String = expression.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        return Err(DiceError::Invalid("empty".into()));
    }

    // split preserving sign
    let mut parts: Vec<(i32, String)> = Vec::new();
    let mut sign = 1i32;
    let mut buf = String::new();
    for (i, ch) in cleaned.chars().enumerate() {
        match ch {
            '+' | '-' if i > 0 => {
                if !buf.is_empty() {
                    parts.push((sign, std::mem::take(&mut buf)));
                }
                sign = if ch == '+' { 1 } else { -1 };
            }
            _ => buf.push(ch),
        }
    }
    if !buf.is_empty() {
        parts.push((sign, buf));
    }

    let mut terms = Vec::new();
    let mut total = 0i32;
    for (sg, raw) in parts {
        if let Some(idx) = raw.find('d').or_else(|| raw.find('D')) {
            let (n_str, rest) = raw.split_at(idx);
            let rest = &rest[1..];
            let n: u32 = if n_str.is_empty() {
                1
            } else {
                n_str.parse().map_err(|_| DiceError::Invalid(raw.clone()))?
            };
            if n == 0 || n > 100 {
                return Err(DiceError::Invalid("too many dice".into()));
            }

            // optional modifier kh/kl/dh/dl
            let (sides_str, keep) = split_keep(rest)?;
            let sides: u32 = sides_str
                .parse()
                .map_err(|_| DiceError::Invalid(raw.clone()))?;
            if sides < 2 || sides > 1000 {
                return Err(DiceError::Invalid("bad sides".into()));
            }

            let rolls: Vec<i32> = (0..n).map(|_| rng.random_range(1..=sides as i32)).collect();
            let kept = apply_keep(&rolls, keep);
            let sum: i32 = kept.iter().sum();
            let value = sg * sum;
            total += value;
            terms.push(RollTerm {
                expr: format!("{}{}", if sg < 0 { "-" } else { "" }, raw),
                kind: TermKind::Dice,
                rolls,
                kept,
                value,
            });
        } else {
            let v: i32 = raw.parse().map_err(|_| DiceError::Invalid(raw.clone()))?;
            let value = sg * v;
            total += value;
            terms.push(RollTerm {
                expr: format!("{}{}", if sg < 0 { "-" } else { "" }, raw),
                kind: TermKind::Modifier,
                rolls: vec![],
                kept: vec![],
                value,
            });
        }
    }

    Ok(RollResult {
        expression: expression.to_string(),
        terms,
        total,
    })
}

#[derive(Clone, Copy)]
enum Keep {
    All,
    KH(usize),
    KL(usize),
    DH(usize),
    DL(usize),
}

fn split_keep(rest: &str) -> Result<(&str, Keep), DiceError> {
    for (op, ctor) in [
        ("kh", Keep::KH as fn(usize) -> Keep),
        ("kl", Keep::KL),
        ("dh", Keep::DH),
        ("dl", Keep::DL),
    ] {
        if let Some(i) = rest.to_ascii_lowercase().find(op) {
            let (l, r) = rest.split_at(i);
            let n_str = &r[op.len()..];
            let n: usize = if n_str.is_empty() {
                1
            } else {
                n_str.parse().map_err(|_| DiceError::Invalid(rest.into()))?
            };
            return Ok((l, ctor(n)));
        }
    }
    Ok((rest, Keep::All))
}

fn apply_keep(rolls: &[i32], keep: Keep) -> Vec<i32> {
    let mut sorted = rolls.to_vec();
    sorted.sort_unstable();
    match keep {
        Keep::All => rolls.to_vec(),
        Keep::KH(n) => sorted.iter().rev().take(n).copied().collect(),
        Keep::KL(n) => sorted.iter().take(n).copied().collect(),
        Keep::DH(n) => sorted
            .iter()
            .take(sorted.len().saturating_sub(n))
            .copied()
            .collect(),
        Keep::DL(n) => sorted
            .iter()
            .rev()
            .take(sorted.len().saturating_sub(n))
            .copied()
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn rng() -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(42)
    }

    #[test]
    fn simple_d20() {
        let r = roll("1d20", &mut rng()).unwrap();
        assert_eq!(r.terms.len(), 1);
        assert!(r.total >= 1 && r.total <= 20);
    }

    #[test]
    fn d20_plus_mod() {
        let r = roll("1d20+5", &mut rng()).unwrap();
        assert_eq!(r.terms.len(), 2);
        assert!(r.total >= 6 && r.total <= 25);
    }

    #[test]
    fn advantage() {
        let r = roll("2d20kh1", &mut rng()).unwrap();
        assert_eq!(r.terms[0].kept.len(), 1);
        assert_eq!(r.terms[0].kept[0], *r.terms[0].rolls.iter().max().unwrap());
    }

    #[test]
    fn disadvantage() {
        let r = roll("2d20kl1", &mut rng()).unwrap();
        assert_eq!(r.terms[0].kept[0], *r.terms[0].rolls.iter().min().unwrap());
    }

    #[test]
    fn fireball() {
        let r = roll("8d6", &mut rng()).unwrap();
        assert_eq!(r.terms[0].rolls.len(), 8);
        assert!(r.total >= 8 && r.total <= 48);
    }

    #[test]
    fn subtraction() {
        let r = roll("1d4-2", &mut rng()).unwrap();
        assert!(r.total >= -1 && r.total <= 2);
    }

    #[test]
    fn rejects_empty() {
        assert!(roll("", &mut rng()).is_err());
    }

    #[test]
    fn rejects_too_many() {
        assert!(roll("101d6", &mut rng()).is_err());
    }

    #[test]
    fn rejects_garbage() {
        assert!(roll("abc", &mut rng()).is_err());
    }
}
