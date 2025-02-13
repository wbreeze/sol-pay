module.exports = {
  async headers() {
    return [
      {
        source: '/actions.json',
        headers: [
          { key: "Access-Control-Allow-Origin",
            value: "*"
          },
          { key: "Access-Control-Allow-Methods",
            value: "GET,POST,PUT,OPTIONS"
          },
          { key: "Access-Control-Allow-Headers",
            value: "Content-Type, Authorization, " +
              "Content-Encoding, Accept-Encoding, " +
              "X-Accept-Action-Version, X-Accept-Blockchain-Ids"
            },
          { key: "Access-Control-Expose-Headers",
            value: "X-Action-Version, X-Blockchain-Ids"
          },
          { key: "Content-Type", value: "application/json" },
        ],
      },
    ]
  },
}
