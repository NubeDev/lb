// The shell root: session-gated navigation. Logged out → Login; logged in → the stack
// (Channels → a room; Workspaces switcher; Extensions list). The active workspace is derived from
// the signed token via useSession — never client state; a switch re-keys every feature hook.

import React from 'react';
import { NavigationContainer } from '@react-navigation/native';
import { createNativeStackNavigator } from '@react-navigation/native-stack';
import type { NativeStackScreenProps } from '@react-navigation/native-stack';
import { Button } from 'react-native';

import { navTheme } from './theme/navigation';
import { LoginScreen } from './features/session/LoginScreen';
import { useSession } from './features/session/useSession';
import { WorkspaceSwitcher } from './features/workspaces/WorkspaceSwitcher';
import { ChannelsScreen } from './features/channels/ChannelsScreen';
import { ChannelScreen } from './features/channels/ChannelScreen';
import { ExtNavList } from './features/ext-host/ExtNavList';

export type ShellStack = {
  Channels: undefined;
  Channel: { channel: string };
  Workspaces: undefined;
  Extensions: undefined;
};

const Stack = createNativeStackNavigator<ShellStack>();

function ChannelsRoute({ navigation }: NativeStackScreenProps<ShellStack, 'Channels'>) {
  return <ChannelsScreen onOpen={(channel) => navigation.navigate('Channel', { channel })} />;
}

function ChannelRoute({ route }: NativeStackScreenProps<ShellStack, 'Channel'>) {
  return <ChannelScreen channel={route.params.channel} />;
}

export default function App(): React.JSX.Element {
  const session = useSession();

  if (!session) return <LoginScreen />;

  return (
    <NavigationContainer theme={navTheme}>
      <Stack.Navigator>
        <Stack.Screen
          name="Channels"
          component={ChannelsRoute}
          options={({ navigation }) => ({
            title: `#${session.workspace}`,
            headerRight: () => (
              <>
                <Button title="Ext" onPress={() => navigation.navigate('Extensions')} />
                <Button title="WS" onPress={() => navigation.navigate('Workspaces')} />
              </>
            ),
          })}
        />
        <Stack.Screen
          name="Channel"
          component={ChannelRoute}
          options={({ route }) => ({ title: `#${route.params.channel}` })}
        />
        <Stack.Screen name="Workspaces" component={WorkspaceSwitcher} />
        <Stack.Screen name="Extensions" component={ExtNavList} />
      </Stack.Navigator>
    </NavigationContainer>
  );
}
