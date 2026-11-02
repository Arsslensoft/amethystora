//! A one-time code in a text (R7-23, the key `codes`): the rule that finds it. It runs on the body of a new text
//! only while the switch is on, and what leaves it is the digits alone, which the program then sends to the shell as the
//! signal `Code`; the body, the sender and the thread never do.
//!
//! **A false code is worse than a missed one.** A code needs a code word beside it, in the languages the
//! catalogs are written in (English, French, German, Spanish, Arabic), as 4 to 8 digits standing alone, and it is
//! never a phone number, a date, money or an order's number. Beside means one of two shapes and nothing looser:
//! the code word, up to four words that hold no digit, the last of them a verb that links (`is`, `est`, `ist`, `es`,
//! `هو`) unless the number follows the word at once, then the number; or the number, that linking verb, a few words
//! with no digit and the code word. Two different numbers that fit are none. A word in another language is no code word.

/// The longest text looked at
const LONGEST: usize = 1024;
/// Words between a code word and the number it names, at most
const BETWEEN: usize = 4;

/// Whether a word is a code word. The word is lower case and without the marks around it
fn code_word(word: &str) -> bool {
    matches!(
        word,
        // English
        "code" | "codes" | "otp" | "passcode" | "password" | "pin"
        // French
        | "passe"
        // German (Passwort, Kennwort, and the compounds with a joining s: Sicherheitscode, Bestätigungscode)
        | "passwort" | "kennwort"
        // Spanish and Portuguese
        | "código" | "codigo" | "clave" | "contraseña"
        // Arabic: the code, the code of yours, the "code" of a message
        | "رمز" | "الرمز" | "رمزك" | "كود" | "الكود" | "كودك"
    ) || (word.chars().count() > 5 && word.ends_with("scode"))
}

/// A verb that links the number to the word: "the code is 482913", "482913 is your code"
fn link(word: &str) -> bool {
    matches!(word, "is" | "are" | "as" | "est" | "sera" | "ist" | "lautet" | "es" | "será" | "هو" | "هي")
}

/// A word with its marks cut off and lower case
fn bare(token: &str) -> String {
    token.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()
}

/// The digits of a token that is a number standing alone: 4 to 8 digits, which may have a mark before and one after
/// and nothing glued to them (a sign, a currency, a slash or a point inside makes a phone number, money or a date)
fn number(token: &str) -> Option<&str> {
    let token = token.trim_start_matches(['(', '#', ':', '=']);
    let token = token.trim_end_matches(['.', ',', ';', ':', '!', '?', ')']);
    ((4..=8).contains(&token.len()) && token.bytes().all(|b| b.is_ascii_digit())).then_some(token)
}

/// Whether a token holds a digit of any script: it is then no plain word between the code word and the number
fn has_digit(token: &str) -> bool {
    token.chars().any(char::is_numeric)
}

/// Whether the token ends a sentence, so that what follows is another one
fn ends_sentence(token: &str) -> bool {
    token.ends_with(['.', '!', '?'])
}

/// The code in a text, as its digits, or `None`
pub fn find_code(text: &str) -> Option<String> {
    if text.len() > LONGEST {
        return None;
    }
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let words: Vec<String> = tokens.iter().map(|t| bare(t)).collect();
    let mut found: Vec<&str> = Vec::new();
    for (at, token) in tokens.iter().enumerate() {
        let Some(digits) = number(token) else { continue };
        if before(&tokens, &words, at) || after(&tokens, &words, at) {
            found.push(digits);
        }
    }
    found.sort_unstable();
    found.dedup();
    match found.as_slice() {
        [one] => Some((*one).to_string()),
        _ => None,
    }
}

/// The code word comes first: "code: 482913", "your verification code is 482913"
fn before(tokens: &[&str], words: &[String], at: usize) -> bool {
    for back in 1..=BETWEEN + 1 {
        let Some(from) = at.checked_sub(back) else { return false };
        if !code_word(&words[from]) {
            // Every word between holds no digit and ends no sentence
            if has_digit(tokens[from]) || ends_sentence(tokens[from]) {
                return false;
            }
            continue;
        }
        // The number follows at once, or the word before it is the linking verb or ends in a colon
        let last = at - 1;
        return last == from || link(&words[last]) || tokens[last].ends_with(':') || tokens[last] == "=" || tokens[last] == "-";
    }
    false
}

/// The number comes first: "482913 is your code", "482913 est votre code"
fn after(tokens: &[&str], words: &[String], at: usize) -> bool {
    // The number ends nothing else: "Born 19900101, code" is no such shape
    if tokens[at].ends_with([',', ';', ':']) {
        return false;
    }
    let Some(verb) = words.get(at + 1) else { return false };
    if !link(verb) {
        return false;
    }
    for ahead in at + 2..=at + 2 + BETWEEN {
        let Some(word) = words.get(ahead) else { return false };
        if code_word(word) {
            return true;
        }
        if has_digit(tokens[ahead]) || tokens[ahead - 1].ends_with([',', ';', '.', '!', '?']) {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_codes_people_get() {
        for (text, code) in [
            ("Your code is 482913", "482913"),
            ("Your verification code is 482913. Do not share it with anyone.", "482913"),
            ("Verification code: 482913", "482913"),
            ("Your code - 482913", "482913"),
            ("Use 482913 as your code", "482913"),
            ("482913 is your login code", "482913"),
            ("Enter the passcode 4821 to continue", "4821"),
            ("OTP 12345678 expires soon", "12345678"),
            ("Votre code de vérification est 482913", "482913"),
            ("482913 est votre code", "482913"),
            ("Ihr Bestätigungscode lautet 482913", "482913"),
            ("Dein Code: 482913", "482913"),
            ("Su código de verificación es 482913", "482913"),
            ("Tu contraseña es 4829", "4829"),
            ("رمز التحقق الخاص بك هو 482913", "482913"),
            ("482913 هو رمز التحقق", "482913"),
            ("Account 123456 code 654321", "654321"),
        ] {
            assert_eq!(find_code(text).as_deref(), Some(code), "{text}");
        }
    }

    #[test]
    fn what_review_and_audit_found_is_no_code() {
        // The audit's examples: six digits that are a balance, an order, a room, a price, a reference
        for text in [
            "Your balance is 125000 TND today",
            "Order 482913 has shipped",
            "Room 482913 tomorrow",
            "Pay 500000 before Friday",
            "ref 000000",
            // A word that says so, and a number that is none
            "Call 98765432 for the code",
            "Promo code SAVE20: get 1000 points",
            "Your password expires in 2026",
            "Born 19900101, code 7",
            // A number and no word that says it is a code
            "Hi, 482913 is a number",
        ] {
            assert_eq!(find_code(text), None, "{text}");
        }
    }

    #[test]
    fn a_phone_number_a_date_money_or_an_order_is_refused_even_beside_the_word() {
        for text in [
            "Your code is +15550100",
            "Your code is 555-0100",
            "Your code is 12/05/2026",
            "Your code is 1,000",
            "Your code is $4829",
            "Your code is 4829.50",
            "Your code is 123456789",
            "Your code is 123",
            "Your code is 4829USD",
            "Your code is #order4829",
        ] {
            assert_eq!(find_code(text), None, "{text}");
        }
    }

    #[test]
    fn two_numbers_are_none_and_a_language_that_is_not_listed_is_none() {
        assert_eq!(find_code("Your code is 482913, not 123456, or code 654321"), None);
        // The same code, with a word in a language the catalogs are not written in
        for text in [
            "Il tuo codice è 482913",
            "Doğrulama kodunuz: 482913",
            "コード 482913",
            "Ваш пароль: 482913",
            "Seu 482913",
        ] {
            assert_eq!(find_code(text), None, "{text}");
        }
        // A word is a word on its own: a longer one that holds it is none (a postcode is no code)
        assert_eq!(find_code("Postcode is 482913"), None);
        assert_eq!(find_code(""), None);
        assert_eq!(find_code(&format!("{} code is 482913", "x ".repeat(600))), None);
    }
}
