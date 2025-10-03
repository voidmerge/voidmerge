async function vm(req) {
  if (req.type === 'fnReq') {
    if (req.body instanceof Uint8Array) {
      req.body = new TextDecoder().decode(req.body);
    }
    return {
      type: 'fnResOk',
      body: `vm received input: ${JSON.stringify(req, null, 2)}`,
      headers: {
        'content-type': 'text/plain',
      }
    };
  } else {
    throw new Error(`invalid type: ${req.type}`);
  }
}
