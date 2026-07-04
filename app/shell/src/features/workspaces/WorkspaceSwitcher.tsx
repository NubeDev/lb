// The workspace switcher: the person's workspaces, current one marked, tap to switch (token
// re-mint / stored-token re-activate). Switching resets nav + caches by construction — every
// feature hook keys off the active session's workspace.

import React from 'react';
import { FlatList, StyleSheet, Text, TouchableOpacity, View } from 'react-native';
import { useSession } from '../session/useSession';
import { useWorkspaces } from './useWorkspaces';

export function WorkspaceSwitcher(): React.JSX.Element {
  const session = useSession();
  const { workspaces, switchTo, error } = useWorkspaces(session?.workspace);

  return (
    <View style={styles.screen}>
      {error !== '' && <Text style={styles.error}>{error}</Text>}
      <FlatList
        data={workspaces}
        keyExtractor={(w) => w.ws}
        renderItem={({ item }) => {
          const active = item.ws === session?.workspace;
          return (
            <TouchableOpacity style={styles.row} onPress={() => void switchTo(item.ws)} disabled={active}>
              <Text style={[styles.name, active && styles.active]}>{item.ws}</Text>
              <Text style={styles.hint}>
                {active ? 'current' : item.stored ? 'stored session' : 'sign-in on switch'}
              </Text>
            </TouchableOpacity>
          );
        }}
        ListEmptyComponent={<Text style={styles.hint}>No workspaces visible.</Text>}
      />
    </View>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, padding: 16 },
  row: { paddingVertical: 14, borderBottomWidth: StyleSheet.hairlineWidth, borderColor: '#ddd' },
  name: { fontSize: 16 },
  active: { fontWeight: '700' },
  hint: { color: '#888', fontSize: 12, marginTop: 2 },
  error: { color: '#c00', marginBottom: 8 },
});
