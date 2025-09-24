async function vm(req) {
  if (req.type === 'fnReq') {
    return {
      type: 'fnResOk',
      body: 'this is a test',
      headers: {
        'content-type': 'text/plain',
      }
    };
  } else {
    throw new Error(`invalid type: ${req.type}`);
  }
}
