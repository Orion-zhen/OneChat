pub(super) fn normalize_english_numbers(text: &str) -> String {
    #[derive(Debug)]
    struct Word<'a> {
        start: usize,
        end: usize,
        text: &'a str,
    }

    let mut words = Vec::new();
    let mut start = None;
    for (index, character) in text.char_indices() {
        if character.is_ascii_alphabetic() {
            start.get_or_insert(index);
        } else if let Some(word_start) = start.take() {
            words.push(Word {
                start: word_start,
                end: index,
                text: &text[word_start..index],
            });
        }
    }
    if let Some(word_start) = start {
        words.push(Word {
            start: word_start,
            end: text.len(),
            text: &text[word_start..],
        });
    }

    let mut output = String::with_capacity(text.len());
    let mut cursor = 0;
    let mut index = 0;
    while index < words.len() {
        if words[index].text == "and" || english_number_value(words[index].text).is_none() {
            index += 1;
            continue;
        }

        let mut end = index + 1;
        while end < words.len()
            && text[words[end - 1].end..words[end].start]
                .chars()
                .all(|character| character.is_whitespace() || character == '-')
            && (words[end].text == "and" || english_number_value(words[end].text).is_some())
        {
            end += 1;
        }
        while end > index + 1 && words[end - 1].text == "and" {
            end -= 1;
        }
        let number_words: Vec<&str> = words[index..end].iter().map(|word| word.text).collect();
        if let Some(value) = parse_english_number(&number_words) {
            output.push_str(&text[cursor..words[index].start]);
            output.push_str(&value.to_string());
            cursor = words[end - 1].end;
            index = end;
        } else {
            index += 1;
        }
    }
    output.push_str(&text[cursor..]);
    output
}

fn english_number_value(word: &str) -> Option<u64> {
    Some(match word {
        "zero" => 0,
        "one" => 1,
        "two" => 2,
        "three" => 3,
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "nine" => 9,
        "ten" => 10,
        "eleven" => 11,
        "twelve" => 12,
        "thirteen" => 13,
        "fourteen" => 14,
        "fifteen" => 15,
        "sixteen" => 16,
        "seventeen" => 17,
        "eighteen" => 18,
        "nineteen" => 19,
        "twenty" => 20,
        "thirty" => 30,
        "forty" => 40,
        "fifty" => 50,
        "sixty" => 60,
        "seventy" => 70,
        "eighty" => 80,
        "ninety" => 90,
        "hundred" => 100,
        "thousand" => 1_000,
        "million" => 1_000_000,
        "billion" => 1_000_000_000,
        _ => return None,
    })
}

fn parse_english_number(words: &[&str]) -> Option<u64> {
    let significant: Vec<&str> = words
        .iter()
        .copied()
        .filter(|word| *word != "and")
        .collect();
    if significant.is_empty() {
        return None;
    }
    if significant.len() > 1
        && significant
            .iter()
            .all(|word| english_number_value(word).is_some_and(|value| value < 10))
    {
        return significant.iter().try_fold(0_u64, |value, word| {
            value
                .checked_mul(10)?
                .checked_add(english_number_value(word)?)
        });
    }

    let mut total = 0_u64;
    let mut current = 0_u64;
    for word in significant {
        let value = english_number_value(word)?;
        match value {
            100 => current = current.max(1).checked_mul(100)?,
            1_000.. => {
                total = total.checked_add(current.max(1).checked_mul(value)?)?;
                current = 0;
            }
            _ => current = current.checked_add(value)?,
        }
    }
    total.checked_add(current)
}

pub(super) fn normalize_chinese_numbers(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut sequence = String::new();
    for character in text.chars().chain(std::iter::once('\0')) {
        if is_chinese_number(character) {
            sequence.push(character);
            continue;
        }
        if !sequence.is_empty() {
            if let Some(value) = parse_chinese_number(&sequence) {
                output.push_str(&value.to_string());
            } else {
                output.push_str(&sequence);
            }
            sequence.clear();
        }
        if character != '\0' {
            output.push(character);
        }
    }
    output
}

fn is_chinese_number(character: char) -> bool {
    matches!(
        character,
        '零' | '〇'
            | '一'
            | '二'
            | '两'
            | '三'
            | '四'
            | '五'
            | '六'
            | '七'
            | '八'
            | '九'
            | '十'
            | '百'
            | '千'
            | '万'
            | '亿'
    )
}

fn chinese_digit(character: char) -> Option<u64> {
    Some(match character {
        '零' | '〇' => 0,
        '一' => 1,
        '二' | '两' => 2,
        '三' => 3,
        '四' => 4,
        '五' => 5,
        '六' => 6,
        '七' => 7,
        '八' => 8,
        '九' => 9,
        _ => return None,
    })
}

fn parse_chinese_number(text: &str) -> Option<u64> {
    if !text
        .chars()
        .any(|character| matches!(character, '十' | '百' | '千' | '万' | '亿'))
    {
        return text.chars().try_fold(0_u64, |value, character| {
            value
                .checked_mul(10)?
                .checked_add(chinese_digit(character)?)
        });
    }

    let mut total = 0_u64;
    let mut section = 0_u64;
    let mut number = 0_u64;
    for character in text.chars() {
        if let Some(digit) = chinese_digit(character) {
            number = digit;
            continue;
        }
        let unit = match character {
            '十' => 10,
            '百' => 100,
            '千' => 1_000,
            '万' => 10_000,
            '亿' => 100_000_000,
            _ => return None,
        };
        if unit < 10_000 {
            section = section.checked_add(number.max(1).checked_mul(unit)?)?;
        } else {
            section = section.checked_add(number)?;
            total = total.checked_add(section.max(1).checked_mul(unit)?)?;
            section = 0;
        }
        number = 0;
    }
    total.checked_add(section)?.checked_add(number)
}
