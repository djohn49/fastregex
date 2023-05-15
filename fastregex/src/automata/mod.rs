use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::Ident;

use regexlib::automata::{Automaton, State};

use crate::automata::advance_function::AdvanceFunction;

mod advance_function;

pub struct EmittableAutomaton {
    state_variables: Vec<StateVariable>,
    state_variable_constructors: Vec<StateVariableConstructor>,
    advance_function: AdvanceFunction,
    terminal_state_names: Vec<Ident>,
    state_variable_names: Vec<Ident>,
}

impl EmittableAutomaton {
    pub fn new(automaton: &Automaton) -> Self {
        let state_variables = automaton
            .states()
            .iter()
            .map(|state| StateVariable {
                name: Ident::new(&format!("state{}", state.id), Span::call_site()),
            })
            .collect();

        let state_variable_constructors = automaton
            .states()
            .iter()
            .map(|state| StateVariableConstructor::new(automaton, state))
            .collect();

        let advance_function = AdvanceFunction::new(automaton);

        let terminal_state_names = automaton
            .terminal_state_ids()
            .iter()
            .map(|id| Ident::new(&format!("state{}", id), Span::call_site()))
            .collect::<Vec<_>>();

        let state_variable_names = automaton.states().iter().map(|state| Ident::new(&format!("state{}", state.id), Span::call_site())).collect();

        Self {
            state_variables,
            state_variable_constructors,
            advance_function,
            terminal_state_names,
            state_variable_names
        }
    }
}

impl ToTokens for EmittableAutomaton {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let state_variables = &self.state_variables;
        let state_variable_constructors = &self.state_variable_constructors;
        let advance_function = &self.advance_function;
        let terminal_state_names = &self.terminal_state_names;
        let state_variable_names = &self.state_variable_names;

        tokens.append_all(quote!(
            struct Automoton {
                #(#state_variables)*
            }

            impl Automoton{

                fn new() -> Self{
                    Self{
                        #(#state_variable_constructors)*
                    }
                }

                #advance_function

                fn is_terminated(&self) -> bool{
                    #(self.#terminal_state_names)||*
                }

                fn is_failed(&self) -> bool{
                    !(#(self.#state_variable_names)||*)
                }
            }

        ));
    }
}

pub struct StateVariable {
    name: Ident,
}

impl ToTokens for StateVariable {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        tokens.append_all(quote!(#name: bool,));
    }
}

pub struct StateVariableConstructor {
    name: Ident,
    default_value: bool,
}

impl StateVariableConstructor {
    fn new(automaton: &Automaton, state: &State) -> Self {
        Self {
            name: Ident::new(&format!("state{}", state.id), Span::call_site()),
            default_value: automaton.start_states().contains(&state.id),
        }
    }
}

impl ToTokens for StateVariableConstructor {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let name = &self.name;
        let default_value = self.default_value;
        tokens.append_all(quote!(#name: #default_value,));
    }
}
