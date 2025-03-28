#!/bin/bash
export PATH=/opt/opendcs/bin:$PATH


mkdir $LRGSHOME/netlist

mv /config/.lrgs.passwd $LRGSHOME/.lrgs.passwd
for user in `cat $LRGSHOME/.lrgs.passwd | cut -d : -f 1 -s`
do
    mkdir -p $LRGSHOME/users/$user
done

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