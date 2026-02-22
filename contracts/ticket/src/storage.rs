use soroban_sdk::{contracttype, Address, Symbol};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum DataKey {
    Ticket(u64),
    OwnerTickets(Address),
    EventTickets(Symbol),
    NextTicketId,
}
