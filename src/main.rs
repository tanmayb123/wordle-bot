use std::error::Error;
use std::fs;
use std::iter::zip;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Instant;
use std::thread;

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
    exists: [i8; 26]
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

fn to_string(chars: [char; 5]) -> String {
    return chars.iter().map(|x| x.to_string()).collect::<Vec<String>>().join("");
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
    let mut exists: [i8; 26] = [0; 26];
    let mut known_exists: [i8; 26] = [0; 26];
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
            exists[*real_char as usize - 97] += 1;
        }
    }
    for index in 0..5 {
        if char_states[index].status == CharacterStatus::CORRECT {
            continue;
        }
        let c = guess[index] as usize - 97;
        if exists[c] > 0 {
            known_exists[c] += 1;
            exists[c] -= 1;
        } else if known_exists[c] > 0 {
            known_exists[c] = -known_exists[c];
        }
    }
    for index in 0..5 {
        let c = guess[index] as usize - 97;
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
    let mut exists: [i8; 26] = [0; 26];
    for i in 0..5 {
        if states.char_states[i].status == CharacterStatus::CORRECT {
            if word[i] != states.char_states[i].character {
                return false;
            }
            continue;
        }
        let c = word[i] as usize - 97;
        if states.exists[c] == -6 {
            return false;
        }
        exists[c] += 1;
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

fn get_expected_value(guess: &[char; 5], words: &Vec<[char; 5]>) -> f32 {
    let mut total_ending_valid = 0;
    for word in words {
        if guess == word {
            continue;
        }
        let result = get_states(guess, word);
        total_ending_valid += total_valid_words(&words, result);
    }
    return (total_ending_valid as f32) / ((words.len() - 1) as f32);
}

fn wordle_worker(guess_recv: Receiver<[char; 5]>, words: Vec<[char; 5]>, value_send: Sender<([char; 5], f32)>) -> Result<(), Box<dyn Error>> {
    loop {
        let guess = guess_recv.recv()?;
        let value = get_expected_value(&guess, &words);
        value_send.send((guess, value))?;
    }
}

fn main() {
    let words = get_allowed_words().unwrap();
    let mut word_senders = Vec::new();
    let (value_sender, value_receiver) = channel();
    for _ in 0..10 {
        let (guess_sender, guess_receiver) = channel();
        word_senders.push(guess_sender);
        let wc = words.clone();
        let vc = value_sender.clone();
        thread::spawn(move|| {
            wordle_worker(guess_receiver, wc, vc);
        });
    }
    let mut i = 0;
    for word in &words {
        word_senders[i].send(word.clone());
        i = (i + 1) % word_senders.len();
    }
    loop {
        let (word, value) = value_receiver.recv().unwrap();
        println!("{} -> {}", to_string(word), value);
    }
}
