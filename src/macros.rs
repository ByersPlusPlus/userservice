/// A macro which automatically generates saving and loading database functions
#[macro_export]
macro_rules! bpp_model_impl {
    ($model_struct:ty, $insert_struct:ty, $primary_key:ident, $primary_key_type:ty, $schema:path, $table_name:ident) => {
        impl $model_struct {
            pub fn get_from_database(check_primary_key: &$primary_key_type, conn: &PgConnection) -> Option<$model_struct> {
                use $schema::*;
                $table_name.filter($primary_key.eq(check_primary_key)).first::<$model_struct>(conn).ok()
            }

            pub fn save_to_database(&self, conn: &PgConnection) -> QueryResult<usize> {
                use $schema::*;
                diesel::update($table_name.filter($primary_key.eq(self.$primary_key))).set(self).execute(conn)
            }
        }

        impl $insert_struct {
            pub fn save_to_database(&self, conn: &PgConnection) -> Option<$model_struct> {
                use $schema::*;
                let new_primary_key: Result<Vec<$primary_key_type>, diesel::result::Error> = diesel::insert_into($table_name).values(self).returning($primary_key).get_results(conn);
                match new_primary_key {
                    Ok(new_primary_key) => {
                        if new_primary_key.len() >= 1 {
                            <$model_struct>::get_from_database(&new_primary_key[0], conn)
                        } else {
                            None
                        }
                    },
                    Err(error) => {
                        use log::error;
                        error!("Error while inserting new {}: {}", stringify!($model_struct), error);
                        None
                    }
                }
            }
        }
    };
    ($model_struct:ty, $primary_key:ident, $primary_key_type:ty, $schema:path, $table_name:ident) => {
        impl $model_struct {
            pub fn get_from_database(check_pk: &$primary_key_type, conn: &PgConnection) -> Option<$model_struct> {
                use $schema::*;
                $table_name.filter($primary_key.eq(check_pk)).first::<$model_struct>(conn).ok()
            }

            pub fn save_to_database(&self, conn: &PgConnection) -> QueryResult<usize> {
                use $schema::*;
                diesel::insert_into($table_name)
                    .values(self)
                    .on_conflict($primary_key)
                    .do_update()
                    .set(self)
                    .execute(conn)
            }
        }
    };
}