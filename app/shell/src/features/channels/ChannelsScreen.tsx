// The channel directory screen: list + create + open a room.

import React, { useState } from 'react';
import { FlatList, StyleSheet, Text, TextInput, TouchableOpacity, View } from 'react-native';
import { useSession } from '../session/useSession';
import { useChannels } from './useChannels';

export function ChannelsScreen({ onOpen }: { onOpen: (channel: string) => void }): React.JSX.Element {
  const session = useSession();
  const { channels, create, error } = useChannels(session?.workspace);
  const [draft, setDraft] = useState('');

  return (
    <View style={styles.screen}>
      {error !== '' && <Text style={styles.error}>{error}</Text>}
      <FlatList
        data={channels}
        keyExtractor={(c) => c.id}
        renderItem={({ item }) => (
          <TouchableOpacity style={styles.row} onPress={() => onOpen(item.id)}>
            <Text style={styles.name}>#{item.id}</Text>
          </TouchableOpacity>
        )}
        ListEmptyComponent={<Text style={styles.hint}>No channels yet — create one below.</Text>}
      />
      <View style={styles.composer}>
        <TextInput
          style={styles.input}
          value={draft}
          onChangeText={setDraft}
          placeholder="new-channel-id"
          autoCapitalize="none"
          autoCorrect={false}
        />
        <TouchableOpacity
          style={styles.button}
          onPress={() => {
            if (draft.trim()) void create(draft.trim());
            setDraft('');
          }}>
          <Text style={styles.buttonText}>Create</Text>
        </TouchableOpacity>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, padding: 16 },
  row: { paddingVertical: 14, borderBottomWidth: StyleSheet.hairlineWidth, borderColor: '#ddd' },
  name: { fontSize: 16 },
  hint: { color: '#888', paddingVertical: 14 },
  error: { color: '#c00', marginBottom: 8 },
  composer: { flexDirection: 'row', gap: 8, paddingTop: 8 },
  input: { flex: 1, borderWidth: 1, borderColor: '#ccc', borderRadius: 8, padding: 10 },
  button: { backgroundColor: '#111', borderRadius: 8, paddingHorizontal: 16, justifyContent: 'center' },
  buttonText: { color: '#fff', fontWeight: '600' },
});
