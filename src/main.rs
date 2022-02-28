use std::error::Error;
use std::fs;
use std::iter::zip;
use std::path::Path;
use std::time::Instant;

#[derive(Debug, Eq, PartialEq)]
enum CharacterStatus {
    CORRECT,
    INCORRECT
}

#[derive(Debug)]
struct CharacterState {
    character: char,
    status: CharacterStatus
}

#[derive(Debug)]
struct GuessResult {
    char_states: [CharacterState; 5],
    exists: [i8; 128]
}

impl CharacterState {
    fn new(character: char) -> CharacterState {
        CharacterState{
            character,
            status: CharacterStatus::INCORRECT
        }
    }
}

fn to_chars<T>(x: T) -> [char; 5] where T: AsRef<str> {
    let mut it = x.as_ref().chars();
    return [
        it.next().unwrap(),
        it.next().unwrap(),
        it.next().unwrap(),
        it.next().unwrap(),
        it.next().unwrap(),
    ];
}

fn get_allowed_words() -> Result<Vec<[char; 5]>, Box<dyn Error>> {
    let words = fs::read_to_string("/Users/tanmaybakshi/wordle/allowed_words.txt")?
        .split("\n")
        .filter(|x| x.len() == 5)
        .map(|x| to_chars(x))
        .collect();
    return Ok(words);
}

fn get_states(guess: &[char; 5], real: &[char; 5]) -> GuessResult {
    let mut exists: [i8; 128] = [0; 128];
    let mut known_exists: [i8; 128] = [0; 128];
    let mut char_states: [CharacterState; 5] = [
        CharacterState::new(guess[0]),
        CharacterState::new(guess[1]),
        CharacterState::new(guess[2]),
        CharacterState::new(guess[3]),
        CharacterState::new(guess[4]),
    ];
    for (index, (guess_char, real_char)) in zip(guess, real).enumerate() {
        if guess_char == real_char {
            char_states[index].status = CharacterStatus::CORRECT;
        } else {
            exists[*real_char as usize] += 1;
        }
    }
    for index in 0..5 {
        if char_states[index].status == CharacterStatus::CORRECT {
            continue;
        }
        let c = guess[index] as usize;
        if exists[c] > 0 {
            known_exists[c] += 1;
            exists[c] -= 1;
        } else if known_exists[c] > 0 {
            known_exists[c] = -known_exists[c];
        }
    }
    for index in 0..5 {
        let c = guess[index] as usize;
        if char_states[index].status == CharacterStatus::INCORRECT && known_exists[c] == 0 {
            known_exists[c] = -6;
        }
    }
    GuessResult{
        char_states,
        exists: known_exists
    }
}

fn word_is_valid(word: &[char; 5], states: &GuessResult) -> bool {
    let mut exists: [i8; 128] = [0; 128];
    for i in 0..5 {
        if states.char_states[i].status == CharacterStatus::CORRECT {
            if word[i] != states.char_states[i].character {
                return false;
            }
            continue;
        }
        if states.exists[word[i] as usize] == -6 {
            return false;
        }
        exists[word[i] as usize] += 1;
    }
    for (ge, re) in zip(exists, states.exists) {
        if re == -6 || re == 0 {
            continue;
        }
        let is_capped = re < 0;
        let total = if is_capped { -re } else { re };
        if (is_capped && ge != total) || (!is_capped && ge < total) {
            return false;
        }
    }
    return true;
}

fn get_valid_words(words: Vec<[char; 5]>, states: GuessResult) -> Vec<[char; 5]> {
    words
        .into_iter()
        .filter(|word| word_is_valid(&word, &states))
        .collect()
}

fn total_valid_words(words: &Vec<[char; 5]>, states: GuessResult) -> usize {
    words
        .iter()
        .filter(|word| word_is_valid(&word, &states))
        .count()
}

fn get_expected_value(guess: [char; 5], words: &Vec<[char; 5]>) -> f32 {
    let mut total_ending_valid = 0;
    for word in words {
        if &guess == word {
            continue;
        }
        let result = get_states(&guess, word);
        total_ending_valid += total_valid_words(&words, result);
    }
    return (total_ending_valid as f32) / ((words.len() - 1) as f32);
}

fn main() {
    let words = get_allowed_words().unwrap();
    println!("{}", words.len());
    let wapple = to_chars("crane");
    let wcrate = to_chars("crate");
    let start = Instant::now();
    let sapple = get_expected_value(wapple, &words);
    let end = start.elapsed().as_millis();
    println!("{}", end);
    let scrate = get_expected_value(wcrate, &words);
    println!("{} {}", sapple, scrate);
}
