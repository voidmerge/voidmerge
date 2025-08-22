# Context

A VoidMerge "context" can be thought of as a database. Multiple data types can be stored in a single context. Those datatypes could be thought of as tables.

## System Types

There are some system types that are predefined in VoidMerge, and the type names all start with "sys":

- "syslogic" - The validation logic for the context
- "sysenv" - Global environment information available in all validation logic
- "sysweb" - Stores an individual file for the built-in static web server
