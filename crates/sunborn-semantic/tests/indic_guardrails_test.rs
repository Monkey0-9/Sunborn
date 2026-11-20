use sunborn_semantic::guardrails::StructuralFingerprint;

#[test]
fn test_indic_hindi_negation_inversion_rejection() {
    let p_allow = "इस लेनदेन ९८४२ को मंजूरी दें"; // Authorize this transaction 9842 (Devanagari digits)
    let p_deny = "इस लेनदेन ९८४२ को मंजूरी मत दें"; // Do NOT authorize this transaction 9842

    let fp_allow = StructuralFingerprint::extract(p_allow);
    let fp_deny = StructuralFingerprint::extract(p_deny);

    assert_eq!(fp_allow.sorted_numeric_literals, vec!["9842"]);
    assert_eq!(fp_deny.sorted_numeric_literals, vec!["9842"]);

    assert!(
        !fp_allow.negation_polarity_bit,
        "p_allow must be affirmative"
    );
    assert!(
        fp_deny.negation_polarity_bit,
        "p_deny must contain negation token 'मत'"
    );

    assert!(
        !fp_allow.matches(&fp_deny),
        "Affirmative and negated Hindi prompts must NEVER match"
    );
}

#[test]
fn test_indic_tamil_negation_inversion_rejection() {
    let p_allow = "பரிவர்த்தனைக்கு ஒப்புதல் அளிக்கவும்";
    let p_deny = "பரிவர்த்தனைக்கு ஒப்புதல் அளிக்க வேண்டாம்";

    let fp_allow = StructuralFingerprint::extract(p_allow);
    let fp_deny = StructuralFingerprint::extract(p_deny);

    assert!(!fp_allow.negation_polarity_bit);
    assert!(fp_deny.negation_polarity_bit);
    assert!(!fp_allow.matches(&fp_deny));
}

#[test]
fn test_indic_telugu_negation_inversion_rejection() {
    let p_allow = "లావాదేవీని ఆమోదించండి";
    let p_deny = "లావాదేవీని ఆమోదించవద్దు";

    let fp_allow = StructuralFingerprint::extract(p_allow);
    let fp_deny = StructuralFingerprint::extract(p_deny);

    assert!(!fp_allow.negation_polarity_bit);
    assert!(fp_deny.negation_polarity_bit);
    assert!(!fp_allow.matches(&fp_deny));
}

#[test]
fn test_indic_bengali_negation_inversion_rejection() {
    let p_allow = "লেনদেন অনুমোদন করুন";
    let p_deny = "লেনদেন অনুমোদন করবেন না";

    let fp_allow = StructuralFingerprint::extract(p_allow);
    let fp_deny = StructuralFingerprint::extract(p_deny);

    assert!(!fp_allow.negation_polarity_bit);
    assert!(fp_deny.negation_polarity_bit);
    assert!(!fp_allow.matches(&fp_deny));
}

#[test]
fn test_indic_devanagari_numeral_shift_rejection() {
    let p1 = "क्रेडिट सीमा ₹५००० तक बढ़ाएं"; // 5000 in Devanagari
    let p2 = "क्रेडिट सीमा ₹५०००० तक बढ़ाएं"; // 50000 in Devanagari

    let fp1 = StructuralFingerprint::extract(p1);
    let fp2 = StructuralFingerprint::extract(p2);

    assert_eq!(fp1.sorted_numeric_literals, vec!["5000"]);
    assert_eq!(fp2.sorted_numeric_literals, vec!["50000"]);
    assert!(
        !fp1.matches(&fp2),
        "Devanagari 5000 and 50000 must not match"
    );
}

#[test]
fn test_indic_hinglish_negation_rejection() {
    let p1 = "Is payment ko approve karo";
    let p2 = "Is payment ko approve mat karo";

    let fp1 = StructuralFingerprint::extract(p1);
    let fp2 = StructuralFingerprint::extract(p2);

    assert!(!fp1.negation_polarity_bit);
    assert!(fp2.negation_polarity_bit);
    assert!(!fp1.matches(&fp2));
}

#[test]
fn test_indic_temporal_shift_rejection() {
    let p_today = "आज का मौसम कैसा है";
    let p_tomorrow = "कल का मौसम कैसा है";

    let fp_today = StructuralFingerprint::extract(p_today);
    let fp_tomorrow = StructuralFingerprint::extract(p_tomorrow);

    assert!(
        !fp_today.matches(&fp_tomorrow),
        "Today and tomorrow in Hindi must not match"
    );
}

#[test]
fn test_indic_quantifier_scope_inversion_rejection() {
    // "सभी कर्मचारी अधिकृत नहीं हैं" (Every employee is not authorized)
    // vs "नहीं, सभी कर्मचारी अधिकृत हैं" (Not every employee is authorized)
    let p1 = "सभी कर्मचारी अधिकृत नहीं हैं";
    let p2 = "नहीं सभी कर्मचारी अधिकृत हैं";

    let fp1 = StructuralFingerprint::extract(p1);
    let fp2 = StructuralFingerprint::extract(p2);

    assert!(
        !fp1.matches(&fp2),
        "Quantifier scope inversion in Hindi ('सभी ... नहीं' vs 'नहीं ... सभी') must not match"
    );
}

#[test]
fn test_indic_role_bound_transposition_rejection() {
    // Financial transfer direction in Hindi: from account 101 to account 202 vs from account 202 to account 101
    let p_a_to_b = "खाता 101 से खाता 202 को ट्रांसफर करें";
    let p_b_to_a = "खाता 202 से खाता 101 को ट्रांसफर करें";

    let fp1 = StructuralFingerprint::extract(p_a_to_b);
    let fp2 = StructuralFingerprint::extract(p_b_to_a);

    assert!(
        !fp1.matches(&fp2),
        "Directional numeric transposition in Hindi ('101 से 202 को' vs '202 से 101 को') must not match"
    );
}
