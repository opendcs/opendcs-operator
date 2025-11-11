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

# TODO: Get all "placeholder. envvars and strip the placeholder. off and make list
# to Apply to below command.

PLACEHOLDERS=()
unset IFS
for var in $(compgen -e); do
    name=$var
    value=${!var}
    if [[ "$name" =~ ^placeholder_.*$ ]]; then
        PLACEHOLDERS+=("-D${name/placeholder_/}=${value}")
    fi
    
done

# Build classpath
CP=$DCSTOOL_HOME/bin/opendcs.jar

# If a user-specific 'dep' (dependencies) directory exists, then
# add all the jars therein to the classpath.
if [ -d "$DCSTOOL_USERDIR/dep" ]; then
  CP=$CP:$DCSTOOL_USERDIR/dep/*
fi
CP=$CP:$DCSTOOL_HOME/dep/*

echo "Placeholders ${PLACEHOLDERS[@]}"
exec java  -Xms120m -cp $CP \
    -Dlogback.configurationFile=$DCSTOOL_HOME/logback.xml \
    -DAPP_NAME=migration \
    -DLOG_LEVEL=${LOG_LEVEL:-INFO} \
    -DDCSTOOL_HOME=$DCSTOOL_HOME -DDECODES_INSTALL_DIR=$DCSTOOL_HOME -DDCSTOOL_USERDIR=$DCSTOOL_USERDIR \
    org.opendcs.database.ManageDatabaseApp -I OpenDCS-Postgres \
    -P /dcs_user_dir/user.properties \
    -username "`cat /secrets/db-admin/username`" \
    -password "`cat /secrets/db-admin/password`" \
    -appUsername "`cat /secrets/db-app/username`" \
    -appPassword "`cat /secrets/db-app/password`" \
    "${PLACEHOLDERS[@]}"