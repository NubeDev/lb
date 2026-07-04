// One channel room: history + live messages + the composer. A capability deny renders as
// "not permitted" — an honest failure, never a silent empty room.

import React from 'react';
import { FlatList, StyleSheet, Text, View } from 'react-native';
import { useSession } from '../session/useSession';
import { useChannel } from './useChannel';
import { MessageComposer } from './MessageComposer';

export function ChannelScreen({ channel }: { channel: string }): React.JSX.Element {
  const session = useSession();
  const { items, post, error } = useChannel(session?.workspace, channel, session?.principal ?? '');

  return (
    <View style={styles.screen}>
      <FlatList
        data={items}
        keyExtractor={(i) => i.id}
        renderItem={({ item }) => (
          <View style={styles.message}>
            <Text style={styles.author}>{item.author}</Text>
            <Text>{item.body}</Text>
          </View>
        )}
        ListEmptyComponent={<Text style={styles.hint}>No messages yet.</Text>}
      />
      {error !== '' && <Text style={styles.error}>{error}</Text>}
      <MessageComposer onSend={(body) => void post(body)} />
    </View>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, padding: 16 },
  message: { paddingVertical: 8 },
  author: { fontWeight: '600', fontSize: 12, color: '#555' },
  hint: { color: '#888', paddingVertical: 14 },
  error: { color: '#c00', paddingVertical: 4 },
});
