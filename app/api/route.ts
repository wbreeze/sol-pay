export async function GET(request: Request) {
  return Response.json({
    api:[
      { methods: [ 'get' ], path: '/api/' },
      { methods: [ 'get' ], path: '/api/tx/' },
      { methods: [ 'post' ], path: '/api/tx/' },
      { methods: [ 'get' ], path: '/photos' },
      { methods: [ 'get' ], path: '/pos' },
    ],
    account: process.env.TX_RECIPIENT_ACCOUNT
  });
}

