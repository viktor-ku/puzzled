import 'dotenv/config'

/**
 * @type { Object.<string, import("knex").Knex.Config> }
 */
export default {
  client: 'pg',
  connection: process.env.DATABASE_URL,
  pool: {
    min: 2,
    max: 10

  },
  migrations: {
    loadExtensions: ['.mjs']
  },
}
