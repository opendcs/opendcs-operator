#!/bin/bash

export DATABASE_URL=`cat /secrets/db-admin/jdbcUrl`
export DATABASE_TYPE=OPENTSDB
export DATABASE_DRIVER="org.postgresql.Driver"
export DATATYPE_STANDARD="CWMS"
export KEYGENERATOR="decodes.sql.SequenceKeyGenerator"

source /opt/opendcs/tsdb_config.sh
echo "***** GENERATED PROPERTIES FILE *****"
cat /dcs_user_dir/user.properties
echo "***** END GENERATED PROPERTIES FILE *****"

exec /opt/opendcs/bin/manageDatabase -I OpenDCS-Postgres \
               -P /dcs_user_dir/user.properties \
               -username "`cat /secrets/db-admin/username`" \
               -password "`cat /secrets/db-admin/password`" \
               -DNUM_TS_TABLES=${NUM_TS_TABLES} \
               -DNUM_TEXT_SCHEMA=${NUM_TEXT_TABLES} \
               -appUsername "`cat /secrets/db-app/username`" \
               -appPassword "`cat /secrets/db-app/password`"