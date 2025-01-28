export async function GET(request: Request) {
  return Response.json([
      { methods: [ 'get' ], path: '/api/' },
      { methods: [ 'get' ], path: '/api/tx/' },
      { methods: [ 'post' ], path: '/api/tx/' },
      { methods: [ 'get' ], path: '/photos' },
  ]);
}

