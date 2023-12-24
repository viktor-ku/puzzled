import { createdAt, id } from "../lib/migrations.js";

/**
 * @param { import("knex").Knex } knex
 * @returns { Promise<void> }
 */
export const up = async (knex) => {
  await knex.schema.createTable('moves', t => {
    id(knex, t)
    t.smallint('nr').notNullable()
    t.string('uci').notNullable()

    t.uuid('game_id').notNullable()
    t.foreign('game_id').references('games.id')
  })
};

/**
 * @param { import("knex").Knex } knex
 * @returns { Promise<void> }
 */
export const down = async (knex) => {
  await knex.schema.dropTable('moves')
};
