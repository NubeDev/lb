// The cap-gated extension nav entries — listed, not yet mountable (the mount contract is the
// app-extensions slice; saying so honestly beats a dead route).

import React from 'react';
import { FlatList, StyleSheet, Text, View } from 'react-native';
import { useSession } from '../session/useSession';
import { useExtensionNav } from './useExtensionNav';

export function ExtNavList(): React.JSX.Element {
  const session = useSession();
  const { entries, loading } = useExtensionNav(session?.workspace, session?.caps ?? []);

  return (
    <View style={styles.screen}>
      <FlatList
        data={entries}
        keyExtractor={(e) => e.ext}
        renderItem={({ item }) => (
          <View style={styles.row}>
            <Text style={styles.name}>{item.ui.label}</Text>
            <Text style={styles.hint}>{item.ext} — opens with the app-extensions slice</Text>
          </View>
        )}
        ListEmptyComponent={
          <Text style={styles.hint}>{loading ? 'Loading…' : 'No extension pages installed.'}</Text>
        }
      />
    </View>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, padding: 16 },
  row: { paddingVertical: 14, borderBottomWidth: StyleSheet.hairlineWidth, borderColor: '#ddd' },
  name: { fontSize: 16 },
  hint: { color: '#888', fontSize: 12, marginTop: 2 },
});
