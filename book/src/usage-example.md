# Usage Example

Take a look at the example1 start script in the monorepo:

<https://github.com/voidmerge/voidmerge/blob/main/ts/example1/package.json>

This runs the "serve-and-push-app" testing convenience command in the `vm` utility.

Note the args that:

- set up a system admin token
- then use that token for pushing the app
- push the `sysenv` from a json file
- push the `syslogic` from a js file
- and finally push all the static web files as `sysweb` entries for the web server

After running this command, you can navigate locally to <http://127.0.0.1:8080> to experience the application.
