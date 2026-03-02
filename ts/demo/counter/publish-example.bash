#!/bin/bash

set -euo pipefail

npm test

export VM_URL="https://demo.voidmerge.com"
export VM_TOKEN="my_ctx_admin_token_here"
export VM_CTX_ADMIN_TOKENS="${VM_TOKEN}"
export VM_CTX="my_ctx_id_here"
export VM_CODE="./dist/bundle-counter.js"
vm ctx-config
