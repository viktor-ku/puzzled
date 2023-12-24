import { createdAt, id } from "../lib/migrations.js";

/**
 * @param { import("knex").Knex } knex
 * @returns { Promise<void> }
 */
export const up = async (knex) => {
  await knex.schema.createTable('games', t => {
    id(knex, t)
    t.text('pgn')
    t.smallint('winner')
    createdAt(knex, t)
  })
};

/**
 * @param { import("knex").Knex } knex
 * @returns { Promise<void> }
 */
export const down = async (knex) => {
  await knex.schema.dropTable('games')
};
