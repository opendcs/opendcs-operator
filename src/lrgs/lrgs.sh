#!/bin/bash
export PATH=/opt/opendcs/bin:$PATH


mkdir $LRGSHOME/netlist

cp /config/.lrgs.passwd $LRGSHOME/.lrgs.passwd
for user in `cat $LRGSHOME/.lrgs.passwd | cut -d : -f 1 -s`
do
    mkdir -p $LRGSHOME/users/$user
done

# Handle DDS config replication
if [ "${LRGS_INDEX}" != "0" ]
then
    LAST_INDEX=`grep number /config/ddsrecv.conf | tail -1 | sed 's/.*"\(\d*\)".*/\1/'`
    if [ "$LAST_INDEX" == "" ]
    then
        LAST_INDEX=-1
    fi
    LAST_INDEX=$((LAST_INDEX+1))

    target_host=`hostname | sed 's/\(.*\)-\d*$/\1-0/'`

    replication_connection="<connection number="$LAST_INDEX" host="$target_host"> \
    <enabled>true</enabled> \
    <port>16003</port> \
    <name>replication</name> \
    <username>replication</username> \
    <authenticate>true</authenticate> \
</connection>"

    if grep "</ddsrecvconf>" /config/ddsrecv.conf
    then
        sed "/<\/ddsrecvconf>/i \
${replication_connection} \
" /config/ddsrecv.conf > /tmp/ddsrecv.conf
    else
        sed "s/<ddsrecvconf \/>/<ddsrecvconf>${replication_connection}<\/ddsrecvconf>/" /config/ddsrecv.conf > /tmp/ddsrecv.conf
    fi
else
    cp /config/ddsrecv.conf /tmp/ddsrecv.conf
fi

DH=$DCSTOOL_HOME

CP=$DH/bin/opendcs.jar

if [ -d "$LRGSHOME/dep" ]
then
    for f in $LRGSHOME/dep/*.jar
    do
    CP=$CP:$f
    done
fi

# Add the OpenDCS standard 3rd party jars to the classpath
for f in `ls $DH/dep/*.jar | sort`
do
    CP=$CP:$f
done

exec java -Xms120m $DECJ_MAXHEAP -cp $CP \
    -DDCSTOOL_HOME=$DH -DDECODES_INSTALL_DIR=$DH \
    -DDCSTOOL_USERDIR=$DCSTOOL_USERDIR -DLRGSHOME=$LRGSHOME \
    lrgs.lrgsmain.LrgsMain -d3 -l /dev/stdout -F -k - $*