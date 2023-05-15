mod advance_function;
mod constructor;
mod is_terminated;
mod state_enum;

use crate::automaton::advance_function::emit_advance_function;
use crate::automaton::constructor::AutomatonConstructor;
use crate::automaton::is_terminated::emit_is_terminated_function;
use crate::automaton::state_enum::StateEnum;
use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use regexlib::automata::Automaton;
use syn::{Lit, LitInt};

pub struct EmittableAutomaton {
    automaton: Automaton,
    state_enum: StateEnum,
    state_count: Lit,
    constructor: AutomatonConstructor,
}

impl EmittableAutomaton {
    pub fn new(automaton: Automaton) -> Self {
        let state_enum = StateEnum::new(&automaton);
        Self {
            constructor: AutomatonConstructor::new(&automaton, &state_enum),
            state_enum,
            state_count: Lit::Int(LitInt::new(
                &format!("{}", automaton.state_count()),
                Span::call_site(),
            )),
            automaton,
        }
    }
}

impl ToTokens for EmittableAutomaton {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let state_enum = &self.state_enum;
        let state_count = &self.state_count;
        let constructor = &self.constructor;
        let advance_function = emit_advance_function(&self.automaton, &self.state_enum);
        let is_terminated = emit_is_terminated_function(&self.automaton, &self.state_enum);

        tokens.append_all(quote!(
            #state_enum

            struct ScratchSpace{
                did_add_state: [usize; #state_count],
                did_add_state_value: usize,
            }

            impl ScratchSpace{
                fn new() -> Self{
                    Self{
                        did_add_state: [0; #state_count],
                        did_add_state_value: 0
                    }
                }
            }

            struct Automaton {
                states: [State; #state_count],
                valid_state_count: usize,
            }

            impl Automaton{

                #constructor

                #advance_function

                #is_terminated

                fn is_failed(&self) -> bool{
                    self.valid_state_count == 0
                }
            }

        ));
    }
}
