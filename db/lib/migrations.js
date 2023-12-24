export const createdAt = (knex, t) =>
  t.datetime('created_at', { precision: 6 }).defaultTo(knex.fn.now(6))

export const id = (knex, t) => {
  t.uuid('id', { primaryKey: true }).defaultTo(knex.fn.uuid())
}
