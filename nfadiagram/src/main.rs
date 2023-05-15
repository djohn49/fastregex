use std::io::Write;
use std::process::{Command, Stdio};

use itertools::Itertools;

use regexlib::automata::{Automaton, TransitionCondition};
use regexlib::parser::character_class::CharacterClass;
use regexlib::parser::RegexEntry;

fn main() {
    let args = std::env::args().collect::<Vec<_>>();

    if args.len() != 4 {
        eprintln!(
            "Usage: {} <regex> <simple svg output> <svg output>",
            args[0]
        );
        std::process::exit(-1);
    }

    let mut args_iter = args.into_iter();
    args_iter.next(); //skip target
    let regex = args_iter.next().unwrap();
    let simple_svg_output_path = args_iter.next().unwrap();
    let svg_output_path = args_iter.next().unwrap();

    let parsed = match RegexEntry::parse(&regex) {
        Ok(parsed) => parsed,
        Err(msg) => {
            eprintln!("Failed to parse regex: {msg}");
            std::process::exit(-1)
        }
    };

    let mut automata = Automaton::from_regex(parsed);
    output_automata(&automata, &svg_output_path);
    automata.simplify();
    output_automata(&automata, &simple_svg_output_path);
}

fn output_automata(automata: &Automaton, file: &str) {
    let graphviz = automata_to_graphviz(&automata);

    let command = Command::new("dot")
        .args(["-Tsvg"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    command
        .stdin
        .as_ref()
        .unwrap()
        .write_all(graphviz.as_bytes())
        .unwrap();
    let output = command.wait_with_output().unwrap();

    std::fs::write(file, output.stdout).unwrap();
}

fn automata_to_graphviz(automata: &Automaton) -> String {
    let mut graphviz = String::new();

    graphviz.push_str("digraph NFA{\n");
    //graphviz.push_str("\trankdir=LR;\n");
    for state_id in 0..automata.state_count() {
        emit_state(automata, state_id, &mut graphviz);
    }

    graphviz.push_str("\tstart [shape=plaintext];\n");
    for start_state in automata.start_states() {
        graphviz.push_str(&format!("\tstart->state{};\n", *start_state));
    }

    graphviz.push_str("}");

    graphviz
}

fn emit_state(automata: &Automaton, state_id: usize, graphviz: &mut String) {
    let state = automata.get_state(state_id);

    //debug name
    graphviz.push_str(&format!(
        "\tstate{} [label=\"{}\",shape={}];\n",
        state_id,
        state_id,
        if automata.is_terminal_state(state_id) {
            "doublecircle"
        } else {
            "oval"
        }
    ));

    //transitions
    for transition in &state.transitions {
        graphviz.push_str(&format!(
            "\tstate{} -> state{} [label=\"{}\"];\n",
            state_id,
            transition.next_state_id,
            transition_to_string(&transition.condition)
        ));
    }
}

fn transition_to_string(transition_condition: &TransitionCondition) -> String {
    match transition_condition {
        TransitionCondition::Epsilon => "ε".into(),
        TransitionCondition::CharacterClass(class) => {
            format!("[{}]", character_class_to_string(class))
        }
        TransitionCondition::Literal(ch) => format!("'{}'", ch),
        TransitionCondition::AnyCharacter => "*".into(),
        TransitionCondition::UnicodeCharacterClass(categories) => categories
            .iter()
            .map(|category| format!("{:?}", category))
            .join(", "),
        TransitionCondition::NegatedUnicodeClass(categories) => format!(
            "!{}",
            categories
                .iter()
                .map(|category| format!("{:?}", category))
                .join(", ")
        ),
    }
}

fn character_class_to_string(character_class: &CharacterClass) -> String {
    match character_class {
        CharacterClass::Negated(class) => format!("^{}", character_class_to_string(class.as_ref())),
        CharacterClass::Char(ch) => format!("{}", *ch),
        CharacterClass::Range { start, end } => format!("{}-{}", *start, *end),
        CharacterClass::Disjunction(classes) => classes
            .iter()
            .map(|class| character_class_to_string(class))
            .collect(),
    }
}
