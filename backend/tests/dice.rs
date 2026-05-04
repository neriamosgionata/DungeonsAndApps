use dungeonsandapps::dice::roll;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

#[test]
fn integration_roll_deterministic() {
    let mut rng = ChaCha8Rng::seed_from_u64(1);
    let r = roll("4d6dl1", &mut rng).unwrap();
    assert_eq!(r.terms[0].rolls.len(), 4);
    assert_eq!(r.terms[0].kept.len(), 3);
    let dropped_low = *r.terms[0].rolls.iter().min().unwrap();
    let kept_sum: i32 = r.terms[0].rolls.iter().sum::<i32>() - dropped_low;
    assert_eq!(r.total, kept_sum);
}
